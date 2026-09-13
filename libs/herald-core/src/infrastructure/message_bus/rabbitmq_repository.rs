use aether_amqp::{Link, Live};
use lapin::options::{BasicPublishOptions, ExchangeDeclareOptions};
use lapin::types::FieldTable;
use lapin::{BasicProperties, Channel, ExchangeKind};

use crate::domain::entities::action::ActionEvent;
use crate::domain::error::HeraldError;
use crate::domain::ports::MessageBusRepository;

/// AMQP delivery mode value for "persistent" messages (survive a broker
/// restart), per the frozen AMQP topology contract.
const PERSISTENT_DELIVERY_MODE: u8 = 2;

/// Publishes [`ActionEvent`]s to a durable topic exchange, using
/// `ActionEvent::routing_key` as the AMQP routing key.
///
/// Ordering across actions of a single deployment is not guaranteed. Delivery
/// is at-least-once: a caller acks a claimed action with the control plane
/// only after a successful publish, so a lost ack simply causes the action to
/// be reclaimed and republished once its lease expires. Consumers must
/// deduplicate on `action_id`.
pub struct RabbitMqMessageBusRepository {
    // A link rather than a channel. lapin does not reconnect, so a channel
    // held from startup stops working the moment the broker restarts or drops
    // an idle connection -- and every publish after that fails with `invalid
    // channel state`, for the life of the process. A data plane in that state
    // claims work from the control plane and silently never delivers it.
    link: Link,
    exchange: String,
}

impl RabbitMqMessageBusRepository {
    /// Connects to RabbitMQ and declares the durable topic exchange actions
    /// are published to.
    ///
    /// Connecting here rather than lazily so a broker that is unreachable at
    /// startup still fails the pod, which is the one signal an operator has.
    pub async fn connect(amqp_url: &str, exchange: impl Into<String>) -> Result<Self, HeraldError> {
        let repository = Self {
            link: Link::to(amqp_url),
            exchange: exchange.into(),
        };

        repository.channel().await?;

        Ok(repository)
    }

    /// A channel with the exchange declared on it.
    ///
    /// Declared whenever the channel is new, because a fresh one carries none
    /// of the topology the old one had. Idempotent at the broker, so a
    /// re-declaration of the same exchange costs one round trip and asserts
    /// nothing changed underneath.
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

        Ok(channel)
    }
}

impl MessageBusRepository for RabbitMqMessageBusRepository {
    async fn publish(&self, event: ActionEvent) -> Result<(), HeraldError> {
        let routing_key = event.routing_key.clone();
        let action_id = event.action_id;

        let payload = serde_json::to_vec(&event).map_err(|err| HeraldError::Internal {
            message: format!("failed to serialize action event {action_id}: {err}"),
        })?;

        let publish = self
            .channel()
            .await?
            .basic_publish(
                &self.exchange,
                &routing_key,
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default().with_delivery_mode(PERSISTENT_DELIVERY_MODE),
            )
            .await
            .map_err(|err| HeraldError::MessageBus {
                message: format!("failed to publish action event {action_id}: {err}"),
            })?;

        publish.await.map_err(|err| HeraldError::MessageBus {
            message: format!("failed to confirm publish of action event {action_id}: {err}"),
        })?;

        Ok(())
    }
}
