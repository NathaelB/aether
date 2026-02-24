use clap::Parser;
use genesis_core::application::dispatcher::EventDispatcher;
use genesis_core::application::handlers::deployment::DeploymentEventHandler;
use genesis_core::domain::ports::EventConsumer;
use genesis_core::infrastructure::rabbitmq::consumer::RabbitMqConsumer;
use std::sync::Arc;
use tracing::info;

use crate::args::Args;

pub mod args;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "genesis=info,genesis_core=info".into()),
        )
        .init();

    dotenvy::dotenv()?;

    let args = Args::parse();
    let queue = args.amqp.amqp_queue.clone();
    let amqp_url = args.amqp.amqp_url.clone();

    info!(%amqp_url, %queue, "starting genesis");

    let handlers: Vec<Arc<dyn genesis_core::domain::ports::EventHandler>> = vec![
        Arc::new(DeploymentEventHandler::new("create")),
        Arc::new(DeploymentEventHandler::new("delete")),
        Arc::new(DeploymentEventHandler::new("update")),
    ];

    let dispatcher = Arc::new(EventDispatcher::new(handlers));
    let consumer = RabbitMqConsumer::new(amqp_url, queue, dispatcher);

    consumer.run().await?;

    Ok(())
}
