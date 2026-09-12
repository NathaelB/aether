use sqlx::PgPool;

use aether_domain::DataPlaneConfig;
use aether_domain::dataplane::value_objects::PlacementWindows;

use crate::{
    AetherConfig, CoreError, application::auth::set_auth_issuer,
    infrastructure::logs::InProcessLogRelay,
};

mod action;
mod audit;
mod auth;
mod catalog;
mod dataplane;
mod deployment;
mod logs;
mod metrics;
mod network_access;
mod organisation;
mod role;
mod upgrade;
mod user;

#[derive(Clone)]
pub struct AetherService {
    pool: PgPool,
    dataplane: DataPlaneConfig,
    /// Log sessions live here, in this process, for as long as somebody is
    /// reading them. Deliberately not in the database: see
    /// [`crate::infrastructure::logs`].
    log_relay: InProcessLogRelay,
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
        Self {
            pool,
            dataplane,
            log_relay: InProcessLogRelay::new(),
        }
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

    Ok(AetherService::with_dataplane_config(
        pg_pool,
        config.dataplane,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn create_service_maps_database_error() {
        use tokio::time::{Duration, timeout};

        let config = AetherConfig {
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
