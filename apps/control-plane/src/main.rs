use std::sync::Arc;

use aether_api::{
    args::Args,
    dns::reconcile_dns_records,
    get_addr, init_logger,
    keys::ensure_wrapping_key,
    objectstore::ensure_archive_bucket,
    operators::{ensure_first_operator, warn_if_nobody_operates},
    purge::purge_deleted_deployments,
    router::router,
    run_server,
    state::state,
};
use clap::Parser;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let args = Arc::new(Args::parse());
    init_logger(&args.log);

    info!("allowed origins: {:?}", args.server.allowed_origins);

    let app_state = state(args.clone()).await?;

    // Before the warning below, and awaited rather than spawned: an
    // installation is not ready to answer a platform request until it is
    // settled who may make one, and the two racing would report that nobody
    // operates an installation that had just been given somebody.
    ensure_first_operator(args.clone(), app_state.clone()).await;
    warn_if_nobody_operates(app_state.clone()).await;

    tokio::spawn(purge_deleted_deployments(app_state.clone()));
    tokio::spawn(ensure_archive_bucket(args.clone()));
    tokio::spawn(ensure_wrapping_key(args.clone()));
    tokio::spawn(reconcile_dns_records(app_state.clone()));

    let router = router(app_state)?;

    let addr = get_addr(&args.server.host, args.server.port).await?;

    run_server(addr, router).await;

    Ok(())
}
