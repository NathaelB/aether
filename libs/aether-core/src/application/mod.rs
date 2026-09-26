use sqlx::PgPool;

use aether_domain::backups::StoreEncryption;
use aether_domain::dataplane::value_objects::PlacementWindows;
use aether_domain::{ArchiveConfig, DataPlaneConfig};

use crate::{
    AetherConfig, CoreError, application::auth::set_auth_issuer,
    infrastructure::logs::InProcessLogRelay,
};

mod action;
mod audit;
mod auth;
mod backup;
mod catalog;
mod dataplane;
mod deployment;
mod dns;
mod invitation;
mod logs;
mod member;
mod metrics;
mod network_access;
mod organisation;
mod platform;
mod role;
mod signals;
mod traces;
mod upgrade;
mod user;

// Re-exported by name rather than as a module:  is already a
// domain module, and two of them under the crate root is a glob collision
// that resolves to whichever the compiler saw first.
pub use platform::FirstOperator;

#[derive(Clone)]
pub struct AetherService {
    pool: PgPool,
    dataplane: DataPlaneConfig,
    archive: ArchiveConfig,
    /// How to administer the realm, when this installation was given a way.
    ///
    /// `None` is an installation that cannot mint a data plane an identity of
    /// its own, which is every installation that has not been reconfigured.
    realm: Option<crate::infrastructure::herald_identity::RealmAdmin>,

    /// Log sessions live here, in this process, for as long as somebody is
    /// reading them. Deliberately not in the database: see
    /// [`crate::infrastructure::logs`].
    log_relay: InProcessLogRelay,

    /// The domain a deployment's own hostname lives under, when this
    /// installation was given one.
    ///
    /// `None` is every installation that has not configured one -- local dev,
    /// or a control plane that has not been pointed at a zone yet -- and
    /// Genesis keeps inventing `.aether.local` for it, exactly today's
    /// behaviour. Independent of `aether-ovh`'s own configuration: the domain
    /// a hostname is decided under and whether this installation also holds
    /// credentials to publish a DNS record into it are two different
    /// questions, decided by the same zone but not required to travel
    /// together.
    domain: Option<String>,
}

/// How long a data plane may go without reporting before placement stops
/// selecting it, when nothing configured it.
///
/// Three times Herald's default poll interval: one missed cycle is a blip,
/// three is a cluster that is gone.
const DEFAULT_DELETED_RETENTION_DAYS: i64 = 30;
const DEFAULT_HEARTBEAT_WINDOW_SECONDS: i64 = 90;
const DEFAULT_PROVISIONING_TIMEOUT_MINUTES: i64 =
    PlacementWindows::DEFAULT_PROVISIONING_TIMEOUT_MINUTES;

impl AetherService {
    /// Uses the default heartbeat window. `create_service` overrides it from
    /// configuration; this exists so a test does not have to build a config to
    /// get a service.
    pub fn new(pool: PgPool) -> Self {
        Self::with_dataplane_config(
            pool,
            DataPlaneConfig {
                heartbeat_window: chrono::Duration::seconds(DEFAULT_HEARTBEAT_WINDOW_SECONDS),
                deleted_retention: chrono::Duration::days(DEFAULT_DELETED_RETENTION_DAYS),
                provisioning_timeout: chrono::Duration::minutes(
                    DEFAULT_PROVISIONING_TIMEOUT_MINUTES,
                ),
            },
        )
    }

    pub fn with_dataplane_config(pool: PgPool, dataplane: DataPlaneConfig) -> Self {
        Self::with_config(pool, dataplane, ArchiveConfig::default())
    }

    pub fn with_config(pool: PgPool, dataplane: DataPlaneConfig, archive: ArchiveConfig) -> Self {
        Self {
            pool,
            dataplane,
            archive,
            realm: None,
            log_relay: InProcessLogRelay::new(),
            domain: None,
        }
    }

    /// The same service, able to mint a data plane an identity of its own.
    pub fn administering(
        mut self,
        realm: Option<crate::infrastructure::herald_identity::RealmAdmin>,
    ) -> Self {
        self.realm = realm;
        self
    }

    /// The same service, publishing a deployment's own hostname under a
    /// domain of its own.
    pub fn with_domain(mut self, domain: Option<String>) -> Self {
        self.domain = domain;
        self
    }

    /// Which data plane a caller speaks for, refusing anybody who is not one.
    ///
    /// Read from the subject its credential carries. Every Herald-facing use
    /// case goes through this, which is the difference between knowing it is
    /// talking to a data plane and knowing which.
    #[aether_macros::transactional(data_plane)]
    pub async fn speaking_data_plane(
        &self,
        identity: &aether_auth::Identity,
    ) -> Result<aether_domain::dataplane::herald_identity::HeraldSpeaking, CoreError> {
        aether_domain::dataplane::herald_identity::speaking_for(&data_plane_repository, identity)
            .await
    }

    /// How to create a client for a data plane, when this installation was
    /// given a way to.
    pub(crate) fn herald_identities(
        &self,
    ) -> Option<crate::infrastructure::herald_identity::FerrisKeyHeraldIdentities> {
        self.realm
            .clone()
            .map(crate::infrastructure::herald_identity::FerrisKeyHeraldIdentities::new)
    }

    pub fn log_relay(&self) -> &InProcessLogRelay {
        &self.log_relay
    }

    pub fn deleted_retention(&self) -> chrono::Duration {
        self.dataplane.deleted_retention
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn heartbeat_window(&self) -> chrono::Duration {
        self.dataplane.heartbeat_window
    }

    /// Where this installation's archives go, if anywhere.
    pub fn archive_config(&self) -> &ArchiveConfig {
        &self.archive
    }

    /// The domain a deployment's own hostname lives under, if this
    /// installation was given one.
    pub(crate) fn deployment_domain(&self) -> Option<&str> {
        self.domain.as_deref()
    }

    pub fn archive_encryption(&self) -> &StoreEncryption {
        &self.archive.encryption
    }

    pub fn placement_windows(&self) -> PlacementWindows {
        PlacementWindows::new(
            self.dataplane.heartbeat_window,
            self.dataplane.provisioning_timeout,
        )
    }
}

pub async fn create_service(config: AetherConfig) -> Result<AetherService, CoreError> {
    let database_url = format!(
        "postgres://{}:{}@{}:{}/{}",
        config.database.username,
        config.database.password,
        config.database.host,
        config.database.port,
        config.database.name
    );

    let pg_pool = PgPool::connect(&database_url)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: e.to_string(),
        })?;
    set_auth_issuer(config.auth.issuer);

    Ok(AetherService::with_config(
        pg_pool,
        config.dataplane,
        config.archive,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn create_service_maps_database_error() {
        use tokio::time::{Duration, timeout};

        let config = AetherConfig {
            archive: ArchiveConfig::default(),
            database: crate::domain::DatabaseConfig {
                host: "127.0.0.1".to_string(),
                port: 1,
                username: "user".to_string(),
                password: "pass".to_string(),
                name: "db".to_string(),
            },
            auth: crate::domain::AuthConfig {
                issuer: "http://issuer.test".to_string(),
            },
            dataplane: DataPlaneConfig {
                heartbeat_window: chrono::Duration::seconds(90),
                deleted_retention: chrono::Duration::days(30),
                provisioning_timeout: chrono::Duration::minutes(30),
            },
        };

        let result = timeout(Duration::from_millis(200), create_service(config)).await;
        assert!(matches!(result, Ok(Err(CoreError::DatabaseError { .. }))) || result.is_err());
    }
}
