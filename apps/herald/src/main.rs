use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use herald_core::domain::entities::dataplane::DataPlaneId;
use herald_core::domain::entities::shard::ShardConfig;
use herald_core::domain::ports::HeraldService;
use herald_core::domain::services::HeraldServiceImpl;
use herald_core::infrastructure::control_plane::auth::ControlPlaneAuth;
use herald_core::infrastructure::control_plane::control_plane_repository::HttpControlPlaneRepository;
use herald_core::infrastructure::logs::kubernetes::KubePodLogSource;
use herald_core::infrastructure::message_bus::outcome_inbox::RabbitMqOutcomeInbox;
use herald_core::infrastructure::message_bus::rabbitmq_repository::RabbitMqMessageBusRepository;
use herald_core::infrastructure::usage::ferriskey::FerriskeyUsageSource;
use herald_core::infrastructure::usage::routing::ProductUsageSource;
use tokio::signal::unix::{SignalKind, signal};
use tokio::time::interval;
use tracing::{error, info, warn};

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

    // Resolved before anything connects: a Herald that starts with no way to
    // authenticate would run its loop and fail every request, which reads as a
    // control plane problem rather than a missing credential.
    let auth = match (
        args.control_plane.auth_issuer.as_deref(),
        args.control_plane.auth_client_secret.as_deref(),
        args.control_plane.control_plane_token.as_deref(),
    ) {
        (Some(issuer), Some(secret), _) => ControlPlaneAuth::client_credentials(
            issuer,
            args.control_plane.auth_client_id.clone(),
            secret,
        ),
        (Some(_), None, _) => {
            return Err("--auth-issuer needs --auth-client-secret".into());
        }
        (None, _, Some(token)) => {
            warn!(
                "using a fixed control plane token; it will stop working when it \
                 expires -- set --auth-issuer and --auth-client-secret instead"
            );
            ControlPlaneAuth::Static(token.to_string())
        }
        (None, _, None) => {
            return Err(
                "set --auth-issuer with --auth-client-secret, or --control-plane-token".into(),
            );
        }
    };

    let control_plane = Arc::new(
        HttpControlPlaneRepository::new(args.control_plane.control_plane_url, auth)
            .with_claim_settings(
                args.control_plane.claim_max,
                args.control_plane.claim_lease_seconds,
            ),
    );

    let message_bus = Arc::new(
        RabbitMqMessageBusRepository::connect(&args.amqp.amqp_url, &args.amqp.amqp_exchange)
            .await?,
    );

    // The queue Genesis's outcomes land in, named after the shard so two
    // Heralds on one cluster do not take each other's reports.
    let outcomes = Arc::new(
        RabbitMqOutcomeInbox::connect(
            &args.amqp.amqp_url,
            &args.amqp.amqp_exchange,
            format!("herald.outcomes.{}", args.sharding.shard_id),
        )
        .await?,
    );

    let dataplane_id = DataPlaneId::new(args.dataplane_id);

    // Checked before the loop starts rather than discovered from an empty
    // series a week later: readings further apart than a bucket is wide cannot
    // be attributed to one minute, so an interval of 60 or more collects
    // nothing at all, silently.
    let usage_interval_seconds = args.usage.usage_interval_seconds;
    if usage_interval_seconds == 0 || usage_interval_seconds >= 60 {
        return Err("--usage-interval-seconds must be between 1 and 59".into());
    }

    let usage_source = Arc::new(ProductUsageSource::new(FerriskeyUsageSource::new(
        args.usage.ferriskey_metrics_url,
    )));

    // Resolved at startup like the credentials above: a Herald that cannot
    // reach its own cluster can still claim and publish everything else, and
    // finding out only when somebody opens a log view would blame the feature
    // for a problem that was there all along.
    let pod_logs = Arc::new(KubePodLogSource::from_env().await?);

    let service = HeraldServiceImpl::new(
        control_plane,
        message_bus,
        outcomes,
        usage_source,
        pod_logs,
        dataplane_id,
        shard_config,
    );

    run(
        service,
        Duration::from_secs(args.poll_interval_seconds),
        Duration::from_secs(usage_interval_seconds),
    )
    .await
}

/// Drives the sync loop on a fixed interval until SIGINT or SIGTERM is
/// received, at which point it returns cleanly.
async fn run<S>(
    service: S,
    poll_interval: Duration,
    usage_interval: Duration,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: HeraldService,
{
    let mut ticker = interval(poll_interval);
    // A second tick in the same loop rather than a task of its own: usage
    // collection then stops on the same signal as everything else, with no
    // handle anyone has to remember to await.
    let mut usage_ticker = interval(usage_interval);
    let mut sigterm = signal(SignalKind::terminate())?;

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if let Err(err) = service.sync_all_deployments().await {
                    error!(error = %err, "sync cycle failed");
                }
            }
            _ = usage_ticker.tick() => {
                if let Err(err) = service.collect_usage().await {
                    error!(error = %err, "usage collection cycle failed");
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
