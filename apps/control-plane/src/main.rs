#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::sync::Arc;

use aether_api::{args::Args, get_addr, init_logger, router::router, run_server, state::state};
use clap::Parser;

#[cfg_attr(coverage_nightly, coverage(off))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();

    let args = Arc::new(Args::parse());
    init_logger(&args.log);

    let app_state = state(args.clone()).await?;

    let router = router(app_state)?;

    let addr = get_addr(&args.server.host, args.server.port).await?;

    run_server(addr, router).await;

    Ok(())
}
