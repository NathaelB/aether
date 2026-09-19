use clap::Parser;
use genesis_core::application::dispatcher::EventDispatcher;
use genesis_core::application::handlers::backup::BackupEventHandler;
use genesis_core::application::handlers::deployment::DeploymentEventHandler;
use genesis_core::application::handlers::drill::DrillEventHandler;
use genesis_core::application::handlers::network_access::NetworkAccessEventHandler;
use genesis_core::application::handlers::upgrade::UpgradeEventHandler;
use genesis_core::domain::ports::{
    DatabaseProbe, EventConsumer, EventHandler, IdentityInstancePort, IdentityInstanceUpgradePort,
    OutcomePublisher,
};
use genesis_core::infrastructure::kubernetes::identity_instance::KubeIdentityInstancePort;
use genesis_core::infrastructure::kubernetes::identity_instance_upgrade::KubeIdentityInstanceUpgradePort;
use genesis_core::infrastructure::kubernetes::status_watcher::IdentityInstanceStatusWatcher;
use genesis_core::infrastructure::postgres::database_probe::SqlxDatabaseProbe;
use genesis_core::infrastructure::rabbitmq::consumer::ACTIONS_EXCHANGE;
use genesis_core::infrastructure::rabbitmq::consumer::RabbitMqConsumer;
use genesis_core::infrastructure::rabbitmq::outcome_publisher::RabbitMqOutcomePublisher;
use kube::Client;
use std::sync::Arc;
use tracing::info;

use crate::args::Args;

pub mod args;

/// Picks the cryptography rustls will use, before anything opens a connection.
///
/// The whole workspace is built as one unit, so the `aws-lc-rs` the operator
/// needs for the AWS SDK is enabled here too, alongside the `ring` the
/// Kubernetes client brings. rustls refuses to guess between two providers and
/// says so by panicking -- at the first handshake, which for this binary is
/// building its Kubernetes client, three lines into `main`.
///
/// See `apps/operator/src/main.rs`, where this was found first.
fn install_crypto_provider() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    install_crypto_provider();

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
    let database: Arc<dyn DatabaseProbe> = Arc::new(SqlxDatabaseProbe);

    let handlers: Vec<Arc<dyn EventHandler>> = vec![
        Arc::new(DeploymentEventHandler::create(identity_instances.clone())),
        Arc::new(DeploymentEventHandler::update(identity_instances.clone())),
        Arc::new(DeploymentEventHandler::restore(identity_instances.clone())),
        Arc::new(DeploymentEventHandler::delete(
            identity_instances.clone(),
            outcomes.clone(),
        )),
        Arc::new(UpgradeEventHandler::new(upgrades.clone())),
        Arc::new(NetworkAccessEventHandler::new(identity_instances.clone())),
        Arc::new(BackupEventHandler::new(identity_instances.clone())),
        Arc::new(DrillEventHandler::new(
            identity_instances.clone(),
            database,
            outcomes.clone(),
        )),
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
