use std::sync::Arc;

use aether_api::{
    args::Args, get_addr, init_logger, objectstore::ensure_archive_bucket,
    purge::purge_deleted_deployments, router::router, run_server, state::state,
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

    tokio::spawn(purge_deleted_deployments(app_state.clone()));
    tokio::spawn(ensure_archive_bucket(args.clone()));

    let router = router(app_state)?;

    let addr = get_addr(&args.server.host, args.server.port).await?;

    run_server(addr, router).await;

    Ok(())
}
