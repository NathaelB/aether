use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use herald_core::domain::entities::dataplane::DataPlaneId;
use herald_core::domain::entities::shard::ShardConfig;
use herald_core::domain::ports::HeraldService;
use herald_core::domain::services::HeraldServiceImpl;
use herald_core::infrastructure::control_plane::control_plane_repository::HttpControlPlaneRepository;
use herald_core::infrastructure::message_bus::rabbitmq_repository::RabbitMqMessageBusRepository;
use tokio::signal::unix::{SignalKind, signal};
use tokio::time::interval;
use tracing::{error, info};

use crate::args::Args;

pub mod args;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "herald=info,herald_core=info".into()),
        )
        .init();

    dotenvy::dotenv().ok();

    let args = Args::parse();

    let shard_config = ShardConfig::new(
        args.sharding.shard_id as usize,
        args.sharding.total_shards as usize,
    );

    info!(
        shard_id = args.sharding.shard_id,
        total_shards = args.sharding.total_shards,
        dataplane_id = %args.dataplane_id,
        control_plane_url = %args.control_plane.control_plane_url,
        amqp_exchange = %args.amqp.amqp_exchange,
        poll_interval_seconds = args.poll_interval_seconds,
        "starting herald"
    );

    let control_plane = Arc::new(
        HttpControlPlaneRepository::new(
            args.control_plane.control_plane_url,
            args.control_plane.control_plane_token,
        )
        .with_claim_settings(
            args.control_plane.claim_max,
            args.control_plane.claim_lease_seconds,
        ),
    );

    let message_bus = Arc::new(
        RabbitMqMessageBusRepository::connect(&args.amqp.amqp_url, args.amqp.amqp_exchange).await?,
    );

    let dataplane_id = DataPlaneId::new(args.dataplane_id);

    let service = HeraldServiceImpl::new(control_plane, message_bus, dataplane_id, shard_config);

    run(service, Duration::from_secs(args.poll_interval_seconds)).await
}

/// Drives the sync loop on a fixed interval until SIGINT or SIGTERM is
/// received, at which point it returns cleanly.
async fn run<S>(service: S, poll_interval: Duration) -> Result<(), Box<dyn std::error::Error>>
where
    S: HeraldService,
{
    let mut ticker = interval(poll_interval);
    let mut sigterm = signal(SignalKind::terminate())?;

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if let Err(err) = service.sync_all_deployments().await {
                    error!(error = %err, "sync cycle failed");
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("received SIGINT, shutting down");
                break;
            }
            _ = sigterm.recv() => {
                info!("received SIGTERM, shutting down");
                break;
            }
        }
    }

    info!("herald stopped");
    Ok(())
}
