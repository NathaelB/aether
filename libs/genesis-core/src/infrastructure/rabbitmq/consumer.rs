use crate::application::dispatcher::EventDispatcher;
use crate::domain::entities::action_event::ActionEvent;
use crate::domain::error::GenesisError;
use crate::domain::ports::EventConsumer;
use lapin::{
    Channel, Connection, ConnectionProperties,
    options::{BasicAckOptions, BasicConsumeOptions, BasicNackOptions, QueueDeclareOptions},
    types::FieldTable,
};
use serde_json::from_slice;
use std::sync::Arc;
use tokio_stream::StreamExt;
use tracing::{error, info};

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
        let conn = Connection::connect(&self.amqp_url, ConnectionProperties::default())
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

            match self.dispatcher.dispatch(event).await {
                Ok(()) => {
                    delivery.ack(BasicAckOptions::default()).await.ok();
                }
                Err(e) => {
                    error!("handler error {e}");
                    delivery
                        .nack(BasicNackOptions {
                            requeue: true,
                            ..Default::default()
                        })
                        .await
                        .ok();
                }
            }
        }

        Ok(())
    }
}
