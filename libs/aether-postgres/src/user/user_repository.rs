use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use aether_domain::{
    CoreError,
    user::{User, UserId, ports::UserRepository},
};
use aether_persistence::{PgExecutor, PgTransaction};

#[derive(FromRow)]
struct UserRow {
    id: Uuid,
    email: String,
    name: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        Self {
            id: UserId(row.id),
            email: row.email,
            name: row.name,

            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub struct PostgresUserRepository<'e, 't> {
    executor: PgExecutor<'e, 't>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'e, 't> PostgresUserRepository<'e, 't> {
    pub fn new(executor: PgExecutor<'e, 't>) -> Self {
        Self { executor }
    }

    pub fn from_tx(tx: &'e PgTransaction<'t>) -> Self {
        Self::new(PgExecutor::from_tx(tx))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'e> PostgresUserRepository<'e, 'e> {
    pub fn from_pool(pool: &'e sqlx::PgPool) -> Self {
        Self::new(PgExecutor::from_pool(pool))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl UserRepository for PostgresUserRepository<'_, '_> {
    async fn insert(&self, user: &User) -> Result<(), CoreError> {
        let now = Utc::now();

        match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query!(
                    r#"
                    INSERT INTO users (id, email, name, created_at, updated_at)
                    VALUES ($1, $2, $3, $4, $5)
                    "#,
                    user.id.0,
                    user.email,
                    user.name,
                    now,
                    now,
                )
                .execute(*pool)
                .await
            }
            PgExecutor::Tx(tx) => {
                let mut guard = tx.lock().await;
                let transaction = guard
                    .as_mut()
                    .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))?;
                sqlx::query!(
                    r#"
                    INSERT INTO users (id, email, name, created_at, updated_at)
                    VALUES ($1, $2, $3, $4, $5)
                    "#,
                    user.id.0,
                    user.email,
                    user.name,
                    now,
                    now,
                )
                .execute(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to insert user: {}", e),
        })?;

        Ok(())
    }
}
