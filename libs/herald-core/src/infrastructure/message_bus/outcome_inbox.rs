use lapin::options::{
    BasicAckOptions, BasicGetOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions,
};
use lapin::types::FieldTable;
use lapin::{Channel, Connection, ConnectionProperties, ExchangeKind};
use serde_json::from_slice;
use tracing::warn;

use crate::domain::entities::outcome::DeploymentOutcomeReport;
use crate::domain::error::HeraldError;
use crate::domain::ports::OutcomeInboxRepository;

/// Bound outside the `deployment.#` namespace Genesis consumes, so the two
/// directions of traffic cannot feed each other on the same exchange.
const OUTCOME_BINDING_KEY: &str = "outcome.#";

pub struct RabbitMqOutcomeInbox {
    _connection: Connection,
    channel: Channel,
    queue: String,
}

impl RabbitMqOutcomeInbox {
    pub async fn connect(
        amqp_url: &str,
        exchange: &str,
        queue: impl Into<String>,
    ) -> Result<Self, HeraldError> {
        let queue = queue.into();

        let connection = Connection::connect(amqp_url, ConnectionProperties::default())
            .await
            .map_err(|err| HeraldError::MessageBus {
                message: format!("failed to connect to RabbitMQ: {err}"),
            })?;

        let channel = connection
            .create_channel()
            .await
            .map_err(|err| HeraldError::MessageBus {
                message: format!("failed to create channel: {err}"),
            })?;

        channel
            .exchange_declare(
                exchange,
                ExchangeKind::Topic,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|err| HeraldError::MessageBus {
                message: format!("failed to declare exchange '{exchange}': {err}"),
            })?;

        // Durable, because an outcome lost to a restart is a deployment stuck
        // in a transient state with nothing left to move it -- the exact bug
        // this queue exists to fix.
        channel
            .queue_declare(
                &queue,
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|err| HeraldError::MessageBus {
                message: format!("failed to declare queue '{queue}': {err}"),
            })?;

        channel
            .queue_bind(
                &queue,
                exchange,
                OUTCOME_BINDING_KEY,
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|err| HeraldError::MessageBus {
                message: format!("failed to bind queue '{queue}': {err}"),
            })?;

        Ok(Self {
            _connection: connection,
            channel,
            queue,
        })
    }
}

impl OutcomeInboxRepository for RabbitMqOutcomeInbox {
    async fn drain(&self, limit: usize) -> Result<Vec<DeploymentOutcomeReport>, HeraldError> {
        let mut reports = Vec::new();

        for _ in 0..limit {
            let delivery = self
                .channel
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
