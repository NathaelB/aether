use sqlx::{PgPool, Postgres, Transaction};
use tokio::sync::Mutex;

use crate::{AetherConfig, CoreError, application::auth::set_auth_issuer};

mod action;
mod auth;
mod deployment;
mod organisation;
mod role;

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

pub(crate) async fn take_transaction<'a, 't>(
    tx: &'a Mutex<Option<Transaction<'t, Postgres>>>,
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
