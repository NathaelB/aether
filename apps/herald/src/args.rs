use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(about, version)]
pub struct Args {
    #[command(flatten)]
    pub(crate) sharding: ShardingArgs,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ShardingArgs {
    #[arg(
        long = "shard-id",
        env = "SHARD_ID",
        default_value = "0",
        name = "Shard ID",
        help = "The ID of this shard (0-indexed)"
    )]
    pub shard_id: u32,

    #[arg(
        long = "total-shards",
        env = "TOTAL_SHARDS",
        default_value = "1",
        name = "Total Shards",
        help = "The total number of shards in the system"
    )]
    pub total_shards: u32,
}
