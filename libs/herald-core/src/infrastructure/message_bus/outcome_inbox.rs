use aether_amqp::{Link, Live};
use lapin::options::{
    BasicAckOptions, BasicGetOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions,
};
use lapin::types::FieldTable;
use lapin::{Channel, ExchangeKind};
use serde_json::from_slice;
use tracing::warn;

use crate::domain::entities::outcome::DeploymentOutcomeReport;
use crate::domain::error::HeraldError;
use crate::domain::ports::OutcomeInboxRepository;

/// Bound outside the `deployment.#` namespace Genesis consumes, so the two
/// directions of traffic cannot feed each other on the same exchange.
const OUTCOME_BINDING_KEY: &str = "outcome.#";

pub struct RabbitMqOutcomeInbox {
    // A link rather than a channel, for the reason the publisher holds one:
    // lapin does not reconnect, and a channel kept from startup stops working
    // the moment the broker restarts or drops an idle connection.
    link: Link,
    exchange: String,
    queue: String,
}

impl RabbitMqOutcomeInbox {
    pub async fn connect(
        amqp_url: &str,
        exchange: &str,
        queue: impl Into<String>,
    ) -> Result<Self, HeraldError> {
        let inbox = Self {
            link: Link::to(amqp_url),
            exchange: exchange.to_string(),
            queue: queue.into(),
        };

        // Connected here rather than lazily so a broker that is unreachable at
        // startup still fails the pod.
        inbox.channel().await?;

        Ok(inbox)
    }

    /// A channel with the exchange, the queue and the binding declared on it.
    ///
    /// All three whenever the channel is new: a fresh channel carries none of
    /// the topology the old one had, and a `basic_get` against a queue this
    /// connection never declared is the same `invalid channel state` in
    /// another costume.
    async fn channel(&self) -> Result<Channel, HeraldError> {
        let live = self
            .link
            .channel()
            .await
            .map_err(|err| HeraldError::MessageBus {
                message: format!("failed to reach RabbitMQ: {err}"),
            })?;

        if let Live::Existing(channel) = live {
            return Ok(channel);
        }

        let channel = live.into_channel();

        channel
            .exchange_declare(
                &self.exchange,
                ExchangeKind::Topic,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|err| HeraldError::MessageBus {
                message: format!("failed to declare exchange '{}': {err}", self.exchange),
            })?;

        // Durable, because an outcome lost to a restart is a deployment stuck
        // in a transient state with nothing left to move it -- the exact bug
        // this queue exists to fix.
        channel
            .queue_declare(
                &self.queue,
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|err| HeraldError::MessageBus {
                message: format!("failed to declare queue '{}': {err}", self.queue),
            })?;

        channel
            .queue_bind(
                &self.queue,
                &self.exchange,
                OUTCOME_BINDING_KEY,
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|err| HeraldError::MessageBus {
                message: format!("failed to bind queue '{}': {err}", self.queue),
            })?;

        Ok(channel)
    }
}

impl OutcomeInboxRepository for RabbitMqOutcomeInbox {
    async fn drain(&self, limit: usize) -> Result<Vec<DeploymentOutcomeReport>, HeraldError> {
        let mut reports = Vec::new();
        // Once per drain, not once per message: a channel that dies mid-drain
        // fails the read, and the next tick re-establishes it.
        let channel = self.channel().await?;

        for _ in 0..limit {
            let delivery = channel
                .basic_get(&self.queue, BasicGetOptions { no_ack: false })
                .await
                .map_err(|err| HeraldError::MessageBus {
                    message: format!("failed to read the outcome queue: {err}"),
                })?;

            let Some(delivery) = delivery else {
                break;
            };

            match from_slice::<DeploymentOutcomeReport>(&delivery.data) {
                Ok(report) => {
                    // Acked before it is reported, deliberately. The
                    // alternative is holding the message until the control
                    // plane confirms, which turns one unreachable control plane
                    // into a queue that never drains. A lost report costs a
                    // status that stays stale; a stuck queue costs every report
                    // behind it.
                    delivery.ack(BasicAckOptions::default()).await.ok();
                    reports.push(report);
                }
                Err(err) => {
                    // Dropped, not requeued: a message this Herald cannot parse
                    // will not parse on the next attempt either, and requeueing
                    // it is the hot loop this project has already met once.
                    warn!(%err, "discarding an unreadable outcome");
                    delivery.ack(BasicAckOptions::default()).await.ok();
                }
            }
        }

        Ok(reports)
    }
}
