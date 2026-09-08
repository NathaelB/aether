use sqlx::PgPool;

use crate::{AetherConfig, CoreError, application::auth::set_auth_issuer};

mod action;
mod auth;
mod dataplane;
mod deployment;
mod organisation;
mod role;
mod user;

#[derive(Clone)]
pub struct AetherService {
    pool: PgPool,
}

impl AetherService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
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

    Ok(AetherService::new(pg_pool))
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
        };

        let result = timeout(Duration::from_millis(200), create_service(config)).await;
        assert!(matches!(result, Ok(Err(CoreError::DatabaseError { .. }))) || result.is_err());
    }
}
