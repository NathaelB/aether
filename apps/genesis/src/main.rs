use clap::Parser;
use genesis_core::application::dispatcher::EventDispatcher;
use genesis_core::application::handlers::deployment::DeploymentEventHandler;
use genesis_core::application::handlers::upgrade::UpgradeEventHandler;
use genesis_core::domain::ports::{
    EventConsumer, EventHandler, IdentityInstancePort, IdentityInstanceUpgradePort,
    OutcomePublisher,
};
use genesis_core::infrastructure::kubernetes::identity_instance::KubeIdentityInstancePort;
use genesis_core::infrastructure::kubernetes::identity_instance_upgrade::KubeIdentityInstanceUpgradePort;
use genesis_core::infrastructure::kubernetes::status_watcher::IdentityInstanceStatusWatcher;
use genesis_core::infrastructure::rabbitmq::consumer::ACTIONS_EXCHANGE;
use genesis_core::infrastructure::rabbitmq::consumer::RabbitMqConsumer;
use genesis_core::infrastructure::rabbitmq::outcome_publisher::RabbitMqOutcomePublisher;
use kube::Client;
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

    dotenvy::dotenv().ok();

    let args = Args::parse();
    let queue = args.amqp.amqp_queue.clone();
    let amqp_url = args.amqp.amqp_url.clone();

    info!(%amqp_url, %queue, "starting genesis");

    let kube = Client::try_default().await?;
    let identity_instances: Arc<dyn IdentityInstancePort> =
        Arc::new(KubeIdentityInstancePort::new(kube.clone()));
    let upgrades: Arc<dyn IdentityInstanceUpgradePort> =
        Arc::new(KubeIdentityInstanceUpgradePort::new(kube.clone()));

    // Its own connection rather than the consumer's channel: the consumer owns
    // its channel for the lifetime of the run loop, and threading a publisher
    // through it would couple two things that fail independently.
    let outcomes: Arc<dyn OutcomePublisher> =
        Arc::new(RabbitMqOutcomePublisher::connect(&amqp_url, ACTIONS_EXCHANGE).await?);

    let handlers: Vec<Arc<dyn EventHandler>> = vec![
        Arc::new(DeploymentEventHandler::create(identity_instances.clone())),
        Arc::new(DeploymentEventHandler::update(identity_instances.clone())),
        Arc::new(DeploymentEventHandler::delete(
            identity_instances.clone(),
            outcomes.clone(),
        )),
        Arc::new(UpgradeEventHandler::new(upgrades.clone())),
    ];

    let dispatcher = Arc::new(EventDispatcher::new(handlers));
    let consumer = RabbitMqConsumer::new(amqp_url, queue, dispatcher);

    // Watching what the operator decided is the other half of Genesis's job,
    // and it is independent of the queue: a broker outage must not stop status
    // being observed, and an empty queue must not mean nothing is watched.
    let watcher = IdentityInstanceStatusWatcher::new(kube, outcomes);

    // Either ending is a Genesis that has stopped doing half its work, so
    // neither is survived: the pod exits and Kubernetes restarts it, which is
    // the recovery mechanism that already exists here.
    tokio::select! {
        result = consumer.run() => result?,
        result = watcher.run() => result?,
    }

    Ok(())
}
