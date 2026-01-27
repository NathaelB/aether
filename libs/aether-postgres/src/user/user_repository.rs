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
    sub: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        Self {
            id: UserId(row.id),
            email: row.email,
            name: row.name,
            sub: row.sub,

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
    async fn upsert_by_email(&self, user: &User) -> Result<User, CoreError> {
        let now = Utc::now();

        let row = match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query_as!(
                    UserRow,
                    r#"
                    INSERT INTO users (id, email, name, sub, created_at, updated_at)
                    VALUES ($1, $2, $3, $4, $5, $6)
                    ON CONFLICT (email)
                    DO UPDATE SET name = EXCLUDED.name, sub = EXCLUDED.sub, updated_at = EXCLUDED.updated_at
                    RETURNING id, email, name, sub, created_at, updated_at
                    "#,
                    user.id.0,
                    user.email,
                    user.name,
                    user.sub,
                    now,
                    now,
                )
                .fetch_one(*pool)
                .await
            }
            PgExecutor::Tx(tx) => {
                let mut guard = tx.lock().await;
                let transaction = guard
                    .as_mut()
                    .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))?;
                sqlx::query_as!(
                    UserRow,
                    r#"
                    INSERT INTO users (id, email, name, sub, created_at, updated_at)
                    VALUES ($1, $2, $3, $4, $5, $6)
                    ON CONFLICT (email)
                    DO UPDATE SET name = EXCLUDED.name, sub = EXCLUDED.sub, updated_at = EXCLUDED.updated_at
                    RETURNING id, email, name, sub, created_at, updated_at
                    "#,
                    user.id.0,
                    user.email,
                    user.name,
                    user.sub,
                    now,
                    now,
                )
                .fetch_one(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to upsert user: {}", e),
        })?;

        Ok(row.into())
    }

    async fn find_by_sub(&self, sub: &str) -> Result<Option<User>, CoreError> {
        let row = match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query_as!(
                    UserRow,
                    r#"
                    SELECT id, email, name, sub, created_at, updated_at
                    FROM users
                    WHERE sub = $1
                    "#,
                    sub
                )
                .fetch_optional(*pool)
                .await
            }
            PgExecutor::Tx(tx) => {
                let mut guard = tx.lock().await;
                let transaction = guard
                    .as_mut()
                    .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))?;
                sqlx::query_as!(
                    UserRow,
                    r#"
                    SELECT id, email, name, sub, created_at, updated_at
                    FROM users
                    WHERE sub = $1
                    "#,
                    sub
                )
                .fetch_optional(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to find user by sub: {}", e),
        })?;

        Ok(row.map(Into::into))
    }
}
