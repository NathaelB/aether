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
        long = "operator-version",
        env = "OPERATOR_VERSION",
        help = "The version of the data plane chart this cluster runs, reported with every heartbeat"
    )]
    pub operator_version: Option<String>,

    #[arg(
        long = "poll-interval-seconds",
        env = "POLL_INTERVAL_SECONDS",
        default_value = "15",
        help = "How often to poll the control plane for pending actions"
    )]
    pub poll_interval_seconds: u64,

    #[command(flatten)]
    pub usage: UsageArgs,
}

#[derive(clap::Args, Debug, Clone)]
pub struct UsageArgs {
    #[arg(
        long = "usage-interval-seconds",
        env = "USAGE_INTERVAL_SECONDS",
        default_value = "15",
        help = "How often each managed instance's counters are read. Must stay \
                below 60: usage is aggregated into one-minute buckets, and a \
                reading further apart than a bucket is wide cannot be \
                attributed to a single minute, so it is discarded."
    )]
    pub usage_interval_seconds: u64,

    #[arg(
        long = "ferriskey-metrics-url",
        env = "FERRISKEY_METRICS_URL",
        default_value = herald_core::infrastructure::usage::ferriskey::DEFAULT_METRICS_URL,
        help = "Where to read a FerrisKey instance's Prometheus metrics, with \
                {service} and {namespace} filled in per deployment. The default \
                matches the Service the operator creates."
    )]
    pub ferriskey_metrics_url: String,
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
        help = "Fixed bearer token. Useful for a test or a short local run, and \
                wrong for anything longer: the control plane checks expiry, so a \
                static token stops working with no way to recover. Prefer \
                --auth-issuer with client credentials."
    )]
    pub control_plane_token: Option<String>,

    #[arg(
        long = "auth-issuer",
        env = "AUTH_ISSUER",
        help = "OIDC issuer to obtain a token from, e.g. https://id.example/realms/aether. \
                Required unless --control-plane-token is set."
    )]
    pub auth_issuer: Option<String>,

    #[arg(
        long = "auth-client-id",
        env = "AUTH_CLIENT_ID",
        default_value = "herald-service",
        help = "Client id to authenticate with. The control plane rejects any \
                caller whose client id does not contain herald-service."
    )]
    pub auth_client_id: String,

    #[arg(
        long = "auth-client-secret",
        env = "AUTH_CLIENT_SECRET",
        help = "Client secret for the client credentials grant"
    )]
    pub auth_client_secret: Option<String>,

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
