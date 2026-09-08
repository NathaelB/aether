use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(about, version)]
pub struct Args {
    #[command(flatten)]
    pub(crate) sharding: ShardingArgs,

    #[command(flatten)]
    pub(crate) control_plane: ControlPlaneArgs,

    #[command(flatten)]
    pub(crate) amqp: AmqpArgs,

    #[arg(
        long = "dataplane-id",
        env = "DATAPLANE_ID",
        help = "The id of the dataplane this Herald instance serves"
    )]
    pub dataplane_id: String,

    #[arg(
        long = "poll-interval-seconds",
        env = "POLL_INTERVAL_SECONDS",
        default_value = "15",
        help = "How often to poll the control plane for pending actions"
    )]
    pub poll_interval_seconds: u64,
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

#[derive(clap::Args, Debug, Clone)]
pub struct ControlPlaneArgs {
    #[arg(
        long = "control-plane-url",
        env = "CONTROL_PLANE_URL",
        help = "Base URL of the control plane API, e.g. https://control-plane.internal"
    )]
    pub control_plane_url: String,

    #[arg(
        long = "control-plane-token",
        env = "CONTROL_PLANE_TOKEN",
        help = "Bearer token used to authenticate as herald-service against the control plane"
    )]
    pub control_plane_token: String,

    #[arg(
        long = "claim-max",
        env = "CLAIM_MAX",
        default_value = "50",
        help = "Maximum number of actions claimed per deployment per cycle"
    )]
    pub claim_max: usize,

    #[arg(
        long = "claim-lease-seconds",
        env = "CLAIM_LEASE_SECONDS",
        default_value = "60",
        help = "Lease duration requested when claiming actions"
    )]
    pub claim_lease_seconds: u64,
}

#[derive(clap::Args, Debug, Clone)]
pub struct AmqpArgs {
    #[arg(
        long = "amqp-url",
        env = "AMQP_URL",
        help = "AMQP connection URL, e.g. amqp://guest:guest@localhost:5672/%2f"
    )]
    pub amqp_url: String,

    #[arg(
        long = "amqp-exchange",
        env = "AMQP_EXCHANGE",
        default_value = "aether.actions",
        help = "Durable topic exchange actions are published to"
    )]
    pub amqp_exchange: String,
}
