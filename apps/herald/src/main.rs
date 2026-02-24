use clap::Parser;

use crate::args::Args;

pub mod args;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv()?;

    let args = Args::parse();

    println!("args: {:?}", args);

    Ok(())
}
