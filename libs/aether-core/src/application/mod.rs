use sqlx::{PgPool, Postgres, Transaction};
use tokio::sync::Mutex;

use crate::{AetherConfig, CoreError, application::auth::set_auth_issuer};

mod action;
mod auth;
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

pub(crate) async fn take_transaction<'t>(
    tx: &Mutex<Option<Transaction<'t, Postgres>>>,
) -> Result<Transaction<'t, Postgres>, CoreError> {
    let mut guard = tx.lock().await;
    guard
        .take()
        .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))
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
    use sqlx::{Postgres, Transaction};
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn take_transaction_returns_error_when_missing() {
        let tx: Mutex<Option<Transaction<'static, Postgres>>> = Mutex::new(None);

        let err = super::take_transaction(&tx).await.unwrap_err();
        assert!(matches!(err, CoreError::InternalError(_)));
    }

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
