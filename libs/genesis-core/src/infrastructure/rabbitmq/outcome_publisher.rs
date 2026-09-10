use lapin::options::{BasicPublishOptions, ExchangeDeclareOptions};
use lapin::types::FieldTable;
use lapin::{BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind};
use serde_json::to_vec;
use tracing::info;

use crate::domain::entities::outcome::DeploymentOutcomeReport;
use crate::domain::error::GenesisError;
use crate::domain::ports::{BoxFuture, OutcomePublisher};

/// Routing key outcomes are published under.
///
/// Deliberately outside the `deployment.#` namespace Genesis itself binds to:
/// on the same exchange, an outcome routed as `deployment.something` would come
/// straight back to Genesis, which would fail to parse it as an action and
/// retry it for ever.
const OUTCOME_ROUTING_KEY: &str = "outcome.deployment";

/// Persistent, per the topology contract: an outcome lost to a broker restart
/// is a deployment stuck in a transient state with nothing left to move it.
const PERSISTENT_DELIVERY_MODE: u8 = 2;

pub struct RabbitMqOutcomePublisher {
    // Kept alive: the channel stops working when the connection backing it
    // drops.
    _connection: Connection,
    channel: Channel,
    exchange: String,
}

impl RabbitMqOutcomePublisher {
    pub async fn connect(
        amqp_url: &str,
        exchange: impl Into<String>,
    ) -> Result<Self, GenesisError> {
        let exchange = exchange.into();

        let connection = Connection::connect(amqp_url, ConnectionProperties::default())
            .await
            .map_err(|e| GenesisError::MessageBus {
                message: format!("failed to connect to RabbitMQ: {e}"),
            })?;

        let channel = connection
            .create_channel()
            .await
            .map_err(|e| GenesisError::MessageBus {
                message: format!("failed to create channel: {e}"),
            })?;

        // Declared here as well as by the publisher of actions: whichever
        // component starts first must find the exchange, and declaring an
        // exchange that already exists with the same arguments is a no-op.
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
            .map_err(|e| GenesisError::MessageBus {
                message: format!("failed to declare exchange '{exchange}': {e}"),
            })?;

        Ok(Self {
            _connection: connection,
            channel,
            exchange,
        })
    }
}

impl OutcomePublisher for RabbitMqOutcomePublisher {
    fn publish<'a>(
        &'a self,
        report: DeploymentOutcomeReport,
    ) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async move {
            let payload = to_vec(&report).map_err(|e| GenesisError::MessageBus {
                message: format!("failed to serialise outcome: {e}"),
            })?;

            info!(
                deployment_id = %report.deployment_id,
                outcome = %report.outcome,
                "reporting outcome"
            );

            self.channel
                .basic_publish(
                    &self.exchange,
                    OUTCOME_ROUTING_KEY,
                    BasicPublishOptions::default(),
                    &payload,
                    BasicProperties::default().with_delivery_mode(PERSISTENT_DELIVERY_MODE),
                )
                .await
                .map_err(|e| GenesisError::MessageBus {
                    message: format!("failed to publish outcome: {e}"),
                })?
                .await
                .map_err(|e| GenesisError::MessageBus {
                    message: format!("broker did not confirm the outcome: {e}"),
                })?;

            Ok(())
        })
    }
}
