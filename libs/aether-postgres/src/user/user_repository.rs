use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use aether_domain::{
    CoreError,
    user::{User, UserId, ports::UserRepository},
};
use aether_macros::repository;
use aether_persistence::SharedTx;

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
#[repository(domain = User, backend = Postgres)]
pub struct PostgresUserRepository<'tx> {
    tx: SharedTx<'tx>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'tx> PostgresUserRepository<'tx> {
    pub fn new(tx: &SharedTx<'tx>) -> Self {
        Self { tx: tx.clone() }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl UserRepository for PostgresUserRepository<'_> {
    async fn upsert_by_email(&self, user: &User) -> Result<User, CoreError> {
        let now = Utc::now();
        let mut tx = self.tx.lock().await;

        let row = sqlx::query_as!(
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
        .fetch_one(&mut ***tx)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to upsert user: {}", e),
        })?;

        Ok(row.into())
    }

    async fn find_by_sub(&self, sub: &str) -> Result<Option<User>, CoreError> {
        let mut tx = self.tx.lock().await;

        let row = sqlx::query_as!(
            UserRow,
            r#"
            SELECT id, email, name, sub, created_at, updated_at
            FROM users
            WHERE sub = $1
            "#,
            sub
        )
        .fetch_optional(&mut ***tx)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to find user by sub: {}", e),
        })?;

        Ok(row.map(Into::into))
    }
}
