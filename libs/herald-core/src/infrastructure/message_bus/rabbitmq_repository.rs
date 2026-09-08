use lapin::options::{BasicPublishOptions, ExchangeDeclareOptions};
use lapin::types::FieldTable;
use lapin::{BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind};

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
    // Kept alive for as long as the repository lives: the channel stops
    // working once the connection backing it is dropped.
    _connection: Connection,
    channel: Channel,
    exchange: String,
}

impl RabbitMqMessageBusRepository {
    /// Connects to RabbitMQ and declares the durable topic exchange actions
    /// are published to.
    pub async fn connect(amqp_url: &str, exchange: impl Into<String>) -> Result<Self, HeraldError> {
        let exchange = exchange.into();

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
                &exchange,
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

        Ok(Self {
            _connection: connection,
            channel,
            exchange,
        })
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
            .channel
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
