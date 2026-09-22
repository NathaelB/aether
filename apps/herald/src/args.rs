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
        default_value = "3",
        long_help = "How often to poll the control plane for pending actions. \
                     Lowered from 15s: a sweep claims every owned deployment's \
                     actions in one request rather than one request per \
                     deployment, so the interval no longer has to pay for the \
                     sweep's own cost on a data plane with many deployments."
    )]
    pub poll_interval_seconds: u64,

    #[arg(
        long = "archive-interval-seconds",
        env = "ARCHIVE_INTERVAL_SECONDS",
        default_value = "300",
        long_help = "How often to tell the control plane about the archives this data \
                     plane has taken. Slow on purpose: an archive is an hourly event at \
                     best, reports are idempotent, and a faster tick only relists a \
                     cluster with nothing new to say."
    )]
    pub archive_interval_seconds: u64,

    #[command(flatten)]
    pub usage: UsageArgs,

    #[command(flatten)]
    pub gateway: GatewayArgs,

    #[command(flatten)]
    pub log_index: LogIndexArgs,

    #[command(flatten)]
    pub otlp: OtlpArgs,
}

/// Where the search index #293 provisions lives, for shipping what the live
/// tail already reads (#294).
///
/// Absent means shipping is off and the live tail is unaffected -- the same
/// shape as `gateway_tls_secret_name` above: a Herald not yet configured
/// with one, or a chart not yet updated to pass it, must still start and
/// keep relaying.
#[derive(clap::Args, Debug, Clone)]
pub struct LogIndexArgs {
    #[arg(
        long = "quickwit-url",
        env = "QUICKWIT_URL",
        help = "Base URL of the Quickwit endpoint log lines are shipped to, e.g. \
                http://quickwit:7280. Absent means shipping to the search index is off. \
                The trace pipeline ships to the same instance, under a different index prefix."
    )]
    pub quickwit_url: Option<String>,
}

/// Where the OTLP trace receiver listens, if at all.
///
/// Absent means the receiver never starts -- the same "off entirely unless
/// configured" shape as [`LogIndexArgs::quickwit_url`], since a Herald with
/// nowhere to ship traces has nothing to gain from also accepting them.
#[derive(clap::Args, Debug, Clone)]
pub struct OtlpArgs {
    #[arg(
        long = "otlp-listen-addr",
        env = "OTLP_LISTEN_ADDR",
        help = "Address the OTLP/HTTP trace receiver binds, e.g. 0.0.0.0:4318. Absent \
                means this door is closed, independent of --otlp-grpc-listen-addr below."
    )]
    pub otlp_listen_addr: Option<String>,

    #[arg(
        long = "otlp-grpc-listen-addr",
        env = "OTLP_GRPC_LISTEN_ADDR",
        help = "Address the OTLP/gRPC trace receiver binds, e.g. 0.0.0.0:4317. Absent \
                means this door is closed, independent of --otlp-listen-addr above -- both \
                are the same OTLP ingestion, over the two transports the spec allows, and an \
                exporter that speaks only one of them (FerrisKey's own defaults to gRPC) \
                needs only that one open."
    )]
    pub otlp_grpc_listen_addr: Option<String>,
}

/// Where to find this data plane's own Gateway, to read its address back for
/// the heartbeat.
///
/// Both optional and paired, unlike the operator's `Edge::from_env`: a
/// Herald that never reports a `gateway_address` changes nothing about how
/// it works today, so a chart not yet updated to set these -- or a data
/// plane with no Gateway of its own -- must still start.
#[derive(clap::Args, Debug, Clone)]
pub struct GatewayArgs {
    #[arg(
        long = "gateway-name",
        env = "AETHER_GATEWAY_NAME",
        help = "Name of this data plane's own Gateway, read back for its address"
    )]
    pub gateway_name: Option<String>,

    #[arg(
        long = "gateway-namespace",
        env = "AETHER_GATEWAY_NAMESPACE",
        help = "Namespace of this data plane's own Gateway"
    )]
    pub gateway_namespace: Option<String>,

    /// The Secret this cluster's own Gateway TLS listener reads its
    /// certificate from, kept current from what the heartbeat carries.
    /// Absent means this installation does not manage one -- the Gateway
    /// keeps serving whatever `gateway.tls.secretName` already points at,
    /// unmanaged by Herald.
    #[arg(
        long = "gateway-tls-secret-name",
        env = "GATEWAY_TLS_SECRET_NAME",
        help = "Secret to keep current with the certificate the control plane distributes"
    )]
    pub gateway_tls_secret_name: Option<String>,
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
