use std::path::PathBuf;

use clap::Parser;

use aether_core::{AetherConfig, AuthConfig, DataPlaneConfig, DatabaseConfig};
use url::Url;

/// `Args::default()` is the configuration clap produces from no arguments at
/// all: every group below carries the same defaults its `#[arg]` attributes
/// declare, so this is not a second set of values that can drift from them.
#[derive(Debug, Clone, Parser, Default)]
pub struct Args {
    #[command(flatten)]
    pub log: LogArgs,

    #[command(flatten)]
    pub db: DatabaseArgs,

    #[command(flatten)]
    pub auth: AuthArgs,

    #[command(flatten)]
    pub server: ServerArgs,

    #[command(flatten)]
    pub dataplane: DataPlaneArgs,

    #[command(flatten)]
    pub object_store: ObjectStoreArgs,

    #[command(flatten)]
    pub key_manager: KeyManagerArgs,

    #[command(flatten)]
    pub platform: PlatformArgs,

    #[command(flatten)]
    pub realm: RealmArgs,

    #[command(flatten)]
    pub ovh: OvhArgs,

    #[command(flatten)]
    pub certificate: CertificateArgs,
}

/// How the control plane administers the realm.
///
/// It does one administrative thing: create the client a data plane's Herald
/// authenticates as, when that data plane is registered. Nothing else here
/// touches the identity provider's administration, and the credentials below
/// should be scoped to managing clients rather than to everything -- on a
/// local stack they are the admin account, like everything else.
///
/// Left empty, no data plane gets an identity of its own and the installation
/// says so when one is registered.
#[derive(Debug, Clone, Default, clap::Args)]
pub struct RealmArgs {
    /// The identity provider's base url, without the realm.
    #[arg(long, env = "REALM_ADMIN_URL", default_value = "")]
    pub admin_url: String,

    /// The realm data planes live in.
    #[arg(long, env = "REALM_NAME", default_value = "aether")]
    pub name: String,

    /// The realm the administrator itself lives in, which is not the one being
    /// administered: FerrisKey's admin account is in `master`.
    #[arg(long, env = "REALM_ADMIN_REALM", default_value = "master")]
    pub admin_realm: String,

    #[arg(long, env = "REALM_ADMIN_CLIENT_ID", default_value = "admin-cli")]
    pub admin_client_id: String,

    #[arg(long, env = "REALM_ADMIN_USERNAME", default_value = "")]
    pub admin_username: String,

    #[arg(long, env = "REALM_ADMIN_PASSWORD", default_value = "")]
    pub admin_password: String,
}

impl RealmArgs {
    /// What this installation was given, if anything.
    pub fn admin(&self) -> Option<aether_core::RealmAdmin> {
        aether_core::RealmAdmin {
            base_url: self.admin_url.clone(),
            realm: self.name.clone(),
            admin_realm: self.admin_realm.clone(),
            admin_client_id: self.admin_client_id.clone(),
            admin_username: self.admin_username.clone(),
            admin_password: self.admin_password.clone(),
        }
        .configured()
    }
}

/// Who may operate this installation when nobody does yet.
#[derive(Debug, Clone, Default, clap::Args)]
pub struct PlatformArgs {
    /// The subject granted every platform right at startup, if it holds none.
    ///
    /// A way back in rather than a standing instruction: it never narrows an
    /// operator somebody deliberately narrowed, and an installation past its
    /// first day has no reason to keep setting it. Empty means the database is
    /// the only authority, which is where this is headed.
    #[arg(long, env = "AETHER_BOOTSTRAP_OPERATOR", default_value = "")]
    pub bootstrap_operator: String,
}

impl From<Args> for AetherConfig {
    fn from(value: Args) -> Self {
        Self {
            database: value.db.into(),
            auth: value.auth.into(),
            dataplane: value.dataplane.into(),
            // Reads the same arguments the bucket provisioning does, so an
            // installation cannot end up creating one bucket and telling its
            // data planes to write into another.
            archive: value.object_store.archive_config(),
        }
    }
}

/// Where archives live.
///
/// Every field has a default that works against the RustFS in
/// `docker-compose.yaml`, so a fresh checkout archives somewhere without
/// anybody configuring anything. Pointing this at Scaleway is four
/// environment variables and no code.
#[derive(clap::Args, Debug, Clone)]
pub struct ObjectStoreArgs {
    #[arg(
        long = "object-store-endpoint",
        env = "OBJECT_STORE_ENDPOINT",
        name = "OBJECT_STORE_ENDPOINT",
        long_help = "Where the object store answers. Set for anything that is not AWS \
                     itself, which is every store this platform runs against today. \
                     Leaving it empty means AWS, not \"no object store\": that is what \
                     an empty bucket name is for."
    )]
    pub endpoint: Option<String>,

    #[arg(
        long = "object-store-region",
        env = "OBJECT_STORE_REGION",
        name = "OBJECT_STORE_REGION",
        default_value = "us-east-1",
        long_help = "Signed into every request. Self hosted stores mostly ignore which \
                     region it is and mind very much that there is one, so there is no \
                     default that quietly works in one place and not another."
    )]
    pub region: String,

    #[arg(
        long = "object-store-bucket",
        env = "OBJECT_STORE_BUCKET",
        name = "OBJECT_STORE_BUCKET",
        default_value = "aether-backups",
        long_help = "The bucket archives are written under. One per installation, with \
                     tenants separated by prefix. Empty disables archiving entirely."
    )]
    pub bucket: String,

    #[arg(
        long = "object-store-access-key",
        env = "OBJECT_STORE_ACCESS_KEY",
        name = "OBJECT_STORE_ACCESS_KEY",
        default_value = "aether",
        long_help = "Access key for the object store."
    )]
    pub access_key_id: String,

    #[arg(
        long = "object-store-secret-key",
        env = "OBJECT_STORE_SECRET_KEY",
        name = "OBJECT_STORE_SECRET_KEY",
        default_value = "aetheraether",
        long_help = "Secret key for the object store."
    )]
    pub secret_access_key: String,

    #[arg(
        long = "object-store-path-style",
        env = "OBJECT_STORE_PATH_STYLE",
        name = "OBJECT_STORE_PATH_STYLE",
        default_value = "true",
        long_help = "Address buckets as host/bucket rather than bucket.host. True for \
                     every self hosted store; AWS wants it false. Getting it wrong \
                     produces a DNS failure naming the bucket, which reads like a \
                     permissions problem and is not one."
    )]
    pub force_path_style: bool,

    #[arg(
        long = "object-store-encryption",
        env = "OBJECT_STORE_ENCRYPTION",
        name = "OBJECT_STORE_ENCRYPTION",
        default_value = "managed",
        long_help = "What the store itself does to an object once it has it: managed, \
                     none, or kms:<key id>. The store decrypts on read whichever is \
                     chosen, so this protects the disks under the bucket and does not \
                     lock the provider out. An archive only the customer can read is a \
                     different mechanism and is not this setting."
    )]
    pub encryption: String,
}

/// Where wrapping keys come from.
///
/// Defaults match the OpenBao in `docker-compose.yaml`, so a fresh checkout has
/// a working key manager without anybody configuring one.
#[derive(clap::Args, Debug, Clone)]
pub struct KeyManagerArgs {
    #[arg(
        long = "key-manager-address",
        env = "KEY_MANAGER_ADDRESS",
        name = "KEY_MANAGER_ADDRESS",
        default_value = "http://localhost:8200",
        long_help = "Where the key manager answers. Anything speaking the transit API: \
                     OpenBao, Vault, or a gateway in front of a cloud key manager."
    )]
    pub address: String,

    #[arg(
        long = "key-manager-token",
        env = "KEY_MANAGER_TOKEN",
        name = "KEY_MANAGER_TOKEN",
        default_value = "aether-root",
        long_help = "The token every request carries. The default is the dev mode root \
                     token from docker-compose and is not a credential; a real \
                     installation uses one scoped to generating and decrypting data \
                     keys."
    )]
    pub token: String,

    #[arg(
        long = "key-manager-mount",
        env = "KEY_MANAGER_MOUNT",
        name = "KEY_MANAGER_MOUNT",
        default_value = "transit",
        long_help = "Where the transit engine is mounted."
    )]
    pub mount: String,

    #[arg(
        long = "key-manager-key",
        env = "KEY_MANAGER_KEY",
        name = "KEY_MANAGER_KEY",
        default_value = "aether-backups",
        long_help = "The key data keys are wrapped with. Empty means this installation \
                     wraps nothing, which is a decision rather than a default: it is \
                     logged as one."
    )]
    pub key: String,
}

impl Default for KeyManagerArgs {
    fn default() -> Self {
        Self {
            address: "http://localhost:8200".to_string(),
            token: "aether-root".to_string(),
            mount: "transit".to_string(),
            key: "aether-backups".to_string(),
        }
    }
}

/// Where a deployment's own DNS record is published.
///
/// Left empty, this installation publishes none: a deployment still gets
/// placed and torn down exactly as it does today, it simply gets no hostname
/// of its own. Filling in a zone and OVH credentials is four environment
/// variables and no code, the same way pointing archives at Scaleway is.
#[derive(Debug, Clone, Default, clap::Args)]
pub struct OvhArgs {
    /// Where the OVH API answers. Regional: `eu`, `ca`, or the US endpoint --
    /// there is no default that is right for every account.
    #[arg(
        long = "ovh-endpoint",
        env = "OVH_ENDPOINT",
        default_value = "https://eu.api.ovh.com/1.0"
    )]
    pub endpoint: String,

    #[arg(
        long = "ovh-application-key",
        env = "OVH_APPLICATION_KEY",
        default_value = ""
    )]
    pub application_key: String,

    #[arg(
        long = "ovh-application-secret",
        env = "OVH_APPLICATION_SECRET",
        default_value = ""
    )]
    pub application_secret: String,

    #[arg(
        long = "ovh-consumer-key",
        env = "OVH_CONSUMER_KEY",
        default_value = ""
    )]
    pub consumer_key: String,

    /// The zone deployments get a record in, e.g. `autharie.fr`, and the
    /// domain their own hostname lives under either way -- see
    /// [`OvhArgs::domain`]. Empty means this installation was given neither.
    #[arg(long = "ovh-zone", env = "OVH_ZONE", default_value = "")]
    pub zone: String,
}

impl OvhArgs {
    /// What this installation was given to publish DNS records with, if
    /// anything. Requires every field: a zone with no credentials to sign
    /// requests can decide a hostname but cannot create the record behind it.
    pub fn config(&self) -> Option<aether_ovh::OvhConfig> {
        aether_ovh::OvhConfig {
            endpoint: self.endpoint.clone(),
            application_key: self.application_key.clone(),
            application_secret: self.application_secret.clone(),
            consumer_key: self.consumer_key.clone(),
            zone: self.zone.clone(),
        }
        .configured()
    }

    /// The domain a deployment's own hostname is decided under, if this
    /// installation was given one.
    ///
    /// Only the zone, not the rest of [`OvhArgs::config`]: deciding a
    /// deployment's hostname does not require anything that can sign an OVH
    /// request, only agreement on which domain it lives under. An
    /// installation with a zone but no credentials gets real-looking
    /// hostnames and no record to answer them -- a state worth allowing on
    /// the way to configuring OVH, not one this method rules out.
    pub fn domain(&self) -> Option<String> {
        let zone = self.zone.trim();
        (!zone.is_empty()).then(|| zone.to_string())
    }
}

/// Where the control plane's own wildcard certificate can be read from.
///
/// Left empty, this installation distributes none: a data plane keeps
/// whatever `gateway.tls.secretName` already means without one, exactly
/// today's behaviour. Both fields are required together -- a Secret is
/// meaningless without knowing which namespace it lives in.
#[derive(Debug, Clone, Default, clap::Args)]
pub struct CertificateArgs {
    /// The Secret cert-manager (or whatever issues it) keeps the current
    /// certificate in, e.g. `autharie-fr-tls`.
    #[arg(
        long = "cert-secret-name",
        env = "CERT_SECRET_NAME",
        default_value = ""
    )]
    pub secret_name: String,

    /// The namespace that Secret lives in. Not necessarily the control
    /// plane's own -- in production this certificate is shared with the
    /// Gateway in front of the console and API, which predates this
    /// installation and lives in its own namespace.
    #[arg(
        long = "cert-secret-namespace",
        env = "CERT_SECRET_NAMESPACE",
        default_value = ""
    )]
    pub secret_namespace: String,
}

impl CertificateArgs {
    /// Where to read the current certificate from, if this installation was
    /// given a Secret to read it from at all.
    pub fn configured(&self) -> Option<(String, String)> {
        let name = self.secret_name.trim();
        let namespace = self.secret_namespace.trim();

        (!name.is_empty() && !namespace.is_empty())
            .then(|| (name.to_string(), namespace.to_string()))
    }
}

impl Default for ObjectStoreArgs {
    fn default() -> Self {
        Self {
            endpoint: None,
            region: "us-east-1".to_string(),
            bucket: "aether-backups".to_string(),
            access_key_id: "aether".to_string(),
            secret_access_key: "aetheraether".to_string(),
            force_path_style: true,
            encryption: "managed".to_string(),
        }
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct DataPlaneArgs {
    #[arg(
        long = "dataplane-heartbeat-window",
        env = "DATAPLANE_HEARTBEAT_WINDOW_SECONDS",
        name = "DATAPLANE_HEARTBEAT_WINDOW_SECONDS",
        default_value = "90",
        long_help = "How many seconds a data plane may go without reporting before \
                     placement stops selecting it. Should be a small multiple of \
                     Herald's poll interval: one missed cycle is a blip, three is a \
                     cluster that is gone."
    )]
    pub heartbeat_window_seconds: i64,

    #[arg(
        long = "default-region",
        env = "DEFAULT_REGION",
        name = "DEFAULT_REGION",
        default_value = "local",
        long_help = "Region used when a deployment request does not name one. A \
                     requested region is always honoured; this only fills in a \
                     missing one, and is configuration rather than a constant \
                     buried in the domain."
    )]
    pub default_region: String,

    #[arg(
        long = "deleted-retention-days",
        env = "DELETED_RETENTION_DAYS",
        name = "DELETED_RETENTION_DAYS",
        default_value = "30",
        long_help = "How many days a deployment whose tear-down was confirmed is kept \
                     before its row and its action history are removed. Only applies \
                     once the data plane has confirmed: one still waiting is one that \
                     needs attention, and is never purged. Zero disables the purge."
    )]
    pub deleted_retention_days: i64,

    #[arg(
        long = "provisioning-timeout-minutes",
        env = "PROVISIONING_TIMEOUT_MINUTES",
        name = "PROVISIONING_TIMEOUT_MINUTES",
        default_value = "30",
        long_help = "How long a data plane that has never reported is still believed to \
                     be coming up. A dedicated plane is placed on while provisioning, \
                     because that is the state every one passes through before its \
                     Herald reports; past this, a plane that has still never reported \
                     is not coming up, and placing on it chooses an outcome nobody \
                     wants over an error message."
    )]
    pub provisioning_timeout_minutes: i64,
}

impl Default for DataPlaneArgs {
    fn default() -> Self {
        Self {
            heartbeat_window_seconds: 90,
            default_region: "local".to_string(),
            deleted_retention_days: 30,
            provisioning_timeout_minutes: 30,
        }
    }
}

impl From<DataPlaneArgs> for DataPlaneConfig {
    fn from(value: DataPlaneArgs) -> Self {
        Self {
            heartbeat_window: chrono::Duration::seconds(value.heartbeat_window_seconds),
            deleted_retention: chrono::Duration::days(value.deleted_retention_days),
            provisioning_timeout: chrono::Duration::minutes(value.provisioning_timeout_minutes),
        }
    }
}

impl Default for AuthArgs {
    fn default() -> Self {
        Self {
            issuer: "http://localhost:8888/realms/aether".to_string(),
        }
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct AuthArgs {
    #[arg(
        long = "auth-issuer",
        env = "AUTH_ISSUER",
        name = "AUTH_ISSUER",
        default_value = "http://localhost:8888/realms/aether",
        long_help = "The issuer URL to use for authentication"
    )]
    pub issuer: String,
}

impl From<AuthArgs> for AuthConfig {
    fn from(value: AuthArgs) -> Self {
        Self {
            issuer: value.issuer,
        }
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct LogArgs {
    #[arg(
        long = "log-filter",
        env = "LOG_FILTER",
        name = "LOG_FILTER",
        long_help = "The log filter to use\nhttps://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives",
        default_value = "info"
    )]
    pub filter: String,
    #[arg(
        long = "log-json",
        env = "LOG_JSON",
        name = "LOG_JSON",
        long_help = "Whether to log in JSON format"
    )]
    pub json: bool,
}

impl Default for LogArgs {
    fn default() -> Self {
        Self {
            filter: "info".to_string(),
            json: false,
        }
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct ServerArgs {
    #[arg(
        short,
        long,
        env,
        num_args = 0..,
        value_delimiter = ',',
        long_help = "Comma-separated list of origins allowed by CORS",
    )]
    pub allowed_origins: Vec<String>,
    #[arg(
        short = 'H',
        long = "server-host",
        env = "SERVER_HOST",
        name = "SERVER_HOST",
        default_value = "0.0.0.0",
        long_help = "The host to run the application on"
    )]
    pub host: String,
    #[arg(
        short = 'P',
        long = "server-port",
        env = "SERVER_PORT",
        name = "SERVER_PORT",
        default_value_t = 3456,
        long_help = "The port to run the application on"
    )]
    pub port: u16,
    #[command(flatten)]
    pub tls: Option<ServerTlsArgs>,
}

#[derive(clap::Args, Debug, Clone)]
#[group(requires_all = ["SERVER_TLS_CERT", "SERVER_TLS_KEY"])]
pub struct ServerTlsArgs {
    #[arg(
        long = "server-tls-cert",
        env = "SERVER_TLS_CERT",
        name = "SERVER_TLS_CERT",
        long_help = "Path to the TLS cert file in PEM format",
        required = false
    )]
    pub cert: PathBuf,
    #[arg(
        long = "server-tls-key",
        env = "SERVER_TLS_KEY",
        name = "SERVER_TLS_KEY",
        long_help = "Path to the TLS key file in PEM format",
        required = false
    )]
    pub key: PathBuf,
}

impl Default for ServerArgs {
    fn default() -> Self {
        Self {
            allowed_origins: vec![],
            host: "0.0.0.0".into(),
            port: 3333,
            tls: None,
        }
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct DatabaseArgs {
    #[arg(
        long = "database-host",
        env = "DATABASE_HOST",
        default_value = "localhost",
        name = "DATABASE_HOST",
        long_help = "The database host to use"
    )]
    pub host: String,
    #[arg(
        long = "database-name",
        env = "DATABASE_NAME",
        default_value = "aether",
        name = "DATABASE_NAME",
        long_help = "The database name to use"
    )]
    pub name: String,
    #[arg(
        long = "database-password",
        env = "DATABASE_PASSWORD",
        default_value = "aether",
        name = "DATABASE_PASSWORD",
        long_help = "The database password to use"
    )]
    pub password: String,
    #[arg(
        long = "database-port",
        env = "DATABASE_PORT",
        default_value_t = 5432,
        name = "DATABASE_PORT",
        long_help = "The database port to use"
    )]
    pub port: u16,
    #[arg(
        long = "database-user",
        env = "DATABASE_USER",
        default_value = "aether",
        name = "DATABASE_USER",
        long_help = "The database user to use"
    )]
    pub user: String,
}

impl Default for DatabaseArgs {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            name: "aether".to_string(),
            password: "aether".to_string(),
            port: 5432,
            user: "aether".to_string(),
        }
    }
}

impl From<Url> for DatabaseArgs {
    fn from(value: Url) -> Self {
        Self {
            host: value
                .host()
                .unwrap_or(url::Host::Domain("localhost"))
                .to_string(),
            name: value.path().to_string(),
            password: value.password().unwrap_or("").to_string(),
            port: value.port().unwrap_or(5432),
            user: value.username().to_string(),
        }
    }
}

impl From<DatabaseArgs> for DatabaseConfig {
    fn from(value: DatabaseArgs) -> Self {
        Self {
            host: value.host,
            name: value.name,
            password: value.password,
            port: value.port,
            username: value.user,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_set() {
        let log = LogArgs::default();
        assert_eq!(log.filter, "info");
        assert!(!log.json);

        let db = DatabaseArgs::default();
        assert_eq!(db.host, "localhost");
        assert_eq!(db.name, "aether");
        assert_eq!(db.password, "aether");
        assert_eq!(db.port, 5432);
        assert_eq!(db.user, "aether");

        let server = ServerArgs::default();
        assert_eq!(server.host, "0.0.0.0");
        assert_eq!(server.port, 3333);
        assert!(server.allowed_origins.is_empty());
    }

    #[test]
    fn args_convert_to_config() {
        let args = Args {
            auth: AuthArgs {
                issuer: "http://issuer.test".to_string(),
            },
            ..Args::default()
        };

        let config: AetherConfig = args.clone().into();
        assert_eq!(config.database.host, args.db.host);
        assert_eq!(config.database.name, args.db.name);
        assert_eq!(config.database.username, args.db.user);
        assert_eq!(config.auth.issuer, args.auth.issuer);
    }

    #[test]
    fn no_secret_configured_means_no_certificate_is_distributed() {
        assert!(CertificateArgs::default().configured().is_none());

        assert!(
            CertificateArgs {
                secret_name: "autharie-fr-tls".to_string(),
                secret_namespace: "".to_string(),
            }
            .configured()
            .is_none(),
            "a name with no namespace is not enough to read a Secret"
        );
    }

    #[test]
    fn a_name_and_namespace_together_are_configured() {
        let args = CertificateArgs {
            secret_name: "autharie-fr-tls".to_string(),
            secret_namespace: "default".to_string(),
        };

        assert_eq!(
            args.configured(),
            Some(("autharie-fr-tls".to_string(), "default".to_string()))
        );
    }
}
