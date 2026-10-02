use crate::application::dispatcher::EventDispatcher;
use crate::domain::entities::action_event::ActionEvent;
use crate::domain::error::GenesisError;
use crate::domain::ports::EventConsumer;
use lapin::{
    Channel, ExchangeKind,
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicNackOptions, ExchangeDeclareOptions,
        QueueBindOptions, QueueDeclareOptions,
    },
    types::FieldTable,
};
use serde_json::from_slice;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio_stream::StreamExt;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Durable topic exchange Herald publishes `ActionEvent`s to.
pub const ACTIONS_EXCHANGE: &str = "aether.actions";

/// The routing keys Genesis binds its queue to: deployment lifecycle events,
/// and the actions that target a whole data plane. A key a handler claims that
/// no pattern here matches is published by Herald and dropped by the broker.
pub const BINDING_KEYS: [&str; 2] = ["deployment.#", "dataplane.#"];

/// Whether the broker would route `routing_key` to Genesis' queue, under the
/// topic exchange's rules: words are separated by dots, `*` stands for one
/// word and `#` for any number of them.
pub fn is_bound(routing_key: &str) -> bool {
    let words: Vec<&str> = routing_key.split('.').collect();
    BINDING_KEYS
        .iter()
        .any(|pattern| matches_pattern(&pattern.split('.').collect::<Vec<_>>(), &words))
}

fn matches_pattern(pattern: &[&str], words: &[&str]) -> bool {
    match pattern.split_first() {
        None => words.is_empty(),
        Some((&"#", rest)) => (0..=words.len()).any(|taken| matches_pattern(rest, &words[taken..])),
        Some((&"*", rest)) => !words.is_empty() && matches_pattern(rest, &words[1..]),
        Some((word, rest)) => words.first() == Some(word) && matches_pattern(rest, &words[1..]),
    }
}

/// Shortest wait before a failed event is put back on the queue.
const RETRY_BASE: Duration = Duration::from_secs(1);

/// Longest wait between attempts.
///
/// The failures worth retrying are ones that clear on their own -- a namespace
/// mid-creation, an API server restarting, a webhook not yet ready. Half a
/// minute is long enough to be quiet and short enough that a deployment
/// recovers without anyone watching.
const RETRY_MAX: Duration = Duration::from_secs(30);

/// How long to wait before returning a failed event to the queue.
///
/// Doubling from `RETRY_BASE`, capped at `RETRY_MAX`. Requeueing immediately --
/// which is what this did -- turns a failure that will never clear into a hot
/// loop: eighteen attempts a second, each writing two log lines, for as long as
/// the process lives. A namespace that did not exist produced exactly that, and
/// buried every other line in the log.
fn retry_delay(attempt: u32) -> Duration {
    RETRY_BASE
        .saturating_mul(2_u32.saturating_pow(attempt.min(16)))
        .min(RETRY_MAX)
}

pub struct RabbitMqConsumer {
    amqp_url: String,
    queue: String,
    dispatcher: Arc<EventDispatcher>,
}

impl RabbitMqConsumer {
    pub fn new(
        amqp_url: impl Into<String>,
        queue: impl Into<String>,
        dispatcher: Arc<EventDispatcher>,
    ) -> Self {
        Self {
            amqp_url: amqp_url.into(),
            queue: queue.into(),
            dispatcher,
        }
    }

    async fn connect(&self) -> Result<Channel, GenesisError> {
        let conn = aether_amqp::connect_with_retry(&self.amqp_url, aether_amqp::DEFAULT_BUDGET)
            .await
            .map_err(|e| GenesisError::MessageBus {
                message: format!("failed to connect to RabbitMQ: {e}"),
            })?;

        let channel = conn
            .create_channel()
            .await
            .map_err(|e| GenesisError::MessageBus {
                message: format!("failed to create channel: {e}"),
            })?;

        channel
            .exchange_declare(
                ACTIONS_EXCHANGE,
                ExchangeKind::Topic,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| GenesisError::MessageBus {
                message: format!("failed to declare exchange '{ACTIONS_EXCHANGE}': {e}"),
            })?;

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
            .map_err(|e| GenesisError::MessageBus {
                message: format!("failed to declare queue '{}': {e}", self.queue),
            })?;

        for binding_key in BINDING_KEYS {
            channel
                .queue_bind(
                    &self.queue,
                    ACTIONS_EXCHANGE,
                    binding_key,
                    QueueBindOptions::default(),
                    FieldTable::default(),
                )
                .await
                .map_err(|e| GenesisError::MessageBus {
                    message: format!(
                        "failed to bind queue '{}' to exchange '{ACTIONS_EXCHANGE}' with '{binding_key}': {e}",
                        self.queue
                    ),
                })?;
        }

        Ok(channel)
    }
}

impl EventConsumer for RabbitMqConsumer {
    async fn run(&self) -> Result<(), GenesisError> {
        info!(queue = %self.queue, "starting RabbitMQ consumer");

        let channel = self.connect().await?;

        let mut consumer = channel
            .basic_consume(
                &self.queue,
                "genesis",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| GenesisError::MessageBus {
                message: format!("failed to start consumer: {e}"),
            })?;

        // Per-action, in memory: RabbitMQ marks a delivery as redelivered but
        // does not count attempts, and a dead-letter exchange is a broker-side
        // change this does not need. Losing the counts on restart is fine --
        // the backoff restarts from the bottom, which is the same place a fresh
        // process would start anyway.
        let mut attempts: HashMap<Uuid, u32> = HashMap::new();

        while let Some(delivery) = consumer.next().await {
            let delivery = match delivery {
                Ok(d) => d,
                Err(e) => {
                    error!("error receiving delivery: {e}");
                    continue;
                }
            };

            let event: ActionEvent = match from_slice(&delivery.data) {
                Ok(e) => e,
                Err(e) => {
                    error!("failed to deserialize event: {e}");
                    delivery
                        .nack(BasicNackOptions {
                            requeue: false,
                            ..Default::default()
                        })
                        .await
                        .ok();

                    continue;
                }
            };

            let action_id = event.action_id;

            match self.dispatcher.dispatch(event).await {
                Ok(()) => {
                    attempts.remove(&action_id);
                    delivery.ack(BasicAckOptions::default()).await.ok();
                }
                Err(e) => {
                    let attempt = attempts.entry(action_id).or_insert(0);
                    *attempt = attempt.saturating_add(1);
                    let delay = retry_delay(*attempt);

                    warn!(
                        %action_id,
                        attempt = *attempt,
                        retry_in_seconds = delay.as_secs(),
                        "handler error, requeueing: {e}"
                    );

                    // Spawned, not awaited. Waiting here would hold the
                    // consumer for the whole delay, so one event that keeps
                    // failing would throttle every other event behind it to the
                    // same cadence -- turning a backoff on one deployment into
                    // a stall on all of them.
                    //
                    // The message is still redelivered: nothing is dropped, and
                    // a failure that clears recovers on its own.
                    tokio::spawn(async move {
                        sleep(delay).await;

                        delivery
                            .nack(BasicNackOptions {
                                requeue: true,
                                ..Default::default()
                            })
                            .await
                            .ok();
                    });
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{RETRY_BASE, RETRY_MAX, is_bound, matches_pattern, retry_delay};

    /// The first failure should not wait long: most of what fails here clears
    /// within a second or two.
    #[test]
    fn the_first_retry_waits_the_base_delay() {
        assert_eq!(retry_delay(1), RETRY_BASE * 2);
    }

    #[test]
    fn the_delay_doubles_until_it_reaches_the_cap() {
        assert!(retry_delay(1) < retry_delay(2));
        assert!(retry_delay(2) < retry_delay(3));
        assert_eq!(retry_delay(20), RETRY_MAX);
    }

    /// The bug this exists for. An immediate requeue turned a failure that
    /// would never clear into eighteen attempts a second, each writing two log
    /// lines, for as long as the process lived.
    #[test]
    fn no_attempt_ever_requeues_immediately() {
        for attempt in 0..64 {
            assert!(
                retry_delay(attempt) >= RETRY_BASE,
                "attempt {attempt} waited less than the base delay"
            );
        }
    }

    /// `2^attempt` overflows a u32 long before attempt 64, and a panic in the
    /// consumer loop would take Genesis down over a retry.
    #[test]
    fn a_large_attempt_count_saturates_rather_than_overflowing() {
        assert_eq!(retry_delay(u32::MAX), RETRY_MAX);
    }

    #[test]
    fn deployment_and_data_plane_actions_are_bound() {
        assert!(is_bound("deployment.create"));
        assert!(is_bound("deployment.upgrade"));
        assert!(is_bound("dataplane.upgrade"));
    }

    #[test]
    fn outcomes_and_unknown_namespaces_are_not_bound() {
        assert!(!is_bound("outcome.deployment"));
        assert!(!is_bound("backup.create"));
        assert!(!is_bound("dataplanes.upgrade"));
    }

    #[test]
    fn hash_matches_no_word_and_star_exactly_one() {
        assert!(matches_pattern(&["a", "#"], &["a"]));
        assert!(matches_pattern(&["a", "*"], &["a", "b"]));
        assert!(!matches_pattern(&["a", "*"], &["a"]));
        assert!(!matches_pattern(&["a", "*"], &["a", "b", "c"]));
    }
}
