use chrono::{DateTime, Utc};
use sqlx::FromRow;
use tracing::info;
use uuid::Uuid;

use aether_domain::{
    CoreError,
    organisation::{
        Organisation, OrganisationId,
        commands::CreateOrganisationData,
        ports::OrganisationRepository,
        value_objects::{
            OrganisationLimits, OrganisationName, OrganisationSlug, OrganisationStatus,
        },
    },
    user::UserId,
};
use aether_persistence::{PgExecutor, PgTransaction};

/// Database row representation for organisations table
///
/// This struct maps directly to the database schema and is used with sqlx's `query_as!()` macro.
#[derive(FromRow)]
struct OrganisationRow {
    id: Uuid,
    name: String,
    slug: String,
    owner_id: Uuid,
    status: String,
    plan: String,
    max_instances: i32,
    max_users: i32,
    max_storage_gb: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
}

impl OrganisationRow {
    /// Converts a database row into a domain Organisation entity
    fn into_organisation(self) -> Result<Organisation, CoreError> {
        Ok(Organisation {
            id: OrganisationId(self.id),
            name: OrganisationName::new(self.name)?,
            slug: OrganisationSlug::new(self.slug)?,
            owner_id: UserId(self.owner_id),
            status: self.status.parse()?,
            plan: self.plan.parse()?,
            limits: OrganisationLimits::custom(
                self.max_instances as usize,
                self.max_users as usize,
                self.max_storage_gb as usize,
            ),
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
        })
    }
}

/// PostgreSQL implementation of the OrganisationRepository
///
/// This repository manages the persistence of organisations in a PostgreSQL database.
/// It implements the OrganisationRepository trait from the domain layer.
///
/// # Responsibilities
/// - Generate UUIDs for new organisations
/// - Manage timestamps (created_at, updated_at, deleted_at)
/// - Execute SQL queries for CRUD operations
/// - Map between database rows and domain entities
///
/// # Database Schema
/// The repository expects a table with the following structure:
/// ```sql
/// CREATE TABLE organisations (
///     id UUID PRIMARY KEY,
///     name VARCHAR(100) NOT NULL,
///     slug VARCHAR(50) NOT NULL UNIQUE,
///     owner_id UUID NOT NULL,
///     status VARCHAR(20) NOT NULL,
///     plan VARCHAR(20) NOT NULL,
///     max_instances INTEGER NOT NULL,
///     max_users INTEGER,  -- NULL means unlimited (for Enterprise plan)
///     max_storage_gb INTEGER NOT NULL,
///     created_at TIMESTAMPTZ NOT NULL,
///     updated_at TIMESTAMPTZ NOT NULL,
///     deleted_at TIMESTAMPTZ  -- NULL means not deleted (soft delete)
/// );
/// ```
#[cfg_attr(coverage_nightly, coverage(off))]
pub struct PostgresOrganisationRepository<'e, 't> {
    executor: PgExecutor<'e, 't>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'e, 't> PostgresOrganisationRepository<'e, 't> {
    /// Creates a new PostgresOrganisationRepository
    pub fn new(executor: PgExecutor<'e, 't>) -> Self {
        Self { executor }
    }

    pub fn from_tx(tx: &'e PgTransaction<'t>) -> Self {
        Self::new(PgExecutor::from_tx(tx))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'e> PostgresOrganisationRepository<'e, 'e> {
    pub fn from_pool(pool: &'e sqlx::PgPool) -> Self {
        Self::new(PgExecutor::from_pool(pool))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl OrganisationRepository for PostgresOrganisationRepository<'_, '_> {
    async fn create(&self, data: CreateOrganisationData) -> Result<Organisation, CoreError> {
        let id = OrganisationId::new();
        let now = Utc::now();
        let status = OrganisationStatus::Active;

        info!("Creating organisation with id: {}", id.0);

        match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query!(
                    r#"
                    INSERT INTO organisations (
                        id, name, slug, owner_id, status, plan,
                        max_instances, max_users, max_storage_gb,
                        created_at, updated_at, deleted_at
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                    "#,
                    id.0,
                    data.name.as_str(),
                    data.slug.as_str(),
                    data.owner_id.0,
                    status.to_string(),
                    data.plan.to_string(),
                    data.limits.max_instances as i32,
                    data.limits.max_users as i32,
                    data.limits.max_storage_gb as i32,
                    now,
                    now,
                    None::<DateTime<Utc>>,
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
                    INSERT INTO organisations (
                        id, name, slug, owner_id, status, plan,
                        max_instances, max_users, max_storage_gb,
                        created_at, updated_at, deleted_at
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                    "#,
                    id.0,
                    data.name.as_str(),
                    data.slug.as_str(),
                    data.owner_id.0,
                    status.to_string(),
                    data.plan.to_string(),
                    data.limits.max_instances as i32,
                    data.limits.max_users as i32,
                    data.limits.max_storage_gb as i32,
                    now,
                    now,
                    None::<DateTime<Utc>>,
                )
                .execute(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to create organisation: {}", e),
        })?;

        Ok(Organisation {
            id,
            name: data.name,
            slug: data.slug,
            owner_id: data.owner_id,
            status,
            plan: data.plan,
            limits: data.limits,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        })
    }

    async fn insert_member(
        &self,
        organisation_id: &OrganisationId,
        user_id: &UserId,
    ) -> Result<(), CoreError> {
        let now = Utc::now();
        let member_id = Uuid::new_v4();
        match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query!(
                    r#"
                    INSERT INTO members (id, organisation_id, user_id, created_at, updated_at)
                    VALUES ($1, $2, $3, $4, $5)
                    "#,
                    member_id,
                    organisation_id.0,
                    user_id.0,
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
                    INSERT INTO members (id, organisation_id, user_id, created_at, updated_at)
                    VALUES ($1, $2, $3, $4, $5)
                    "#,
                    member_id,
                    organisation_id.0,
                    user_id.0,
                    now,
                    now,
                )
                .execute(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to insert organisation member: {}", e),
        })?;

        Ok(())
    }

    async fn find_by_id(&self, id: &OrganisationId) -> Result<Option<Organisation>, CoreError> {
        let row = match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query_as!(
                    OrganisationRow,
                    r#"
                    SELECT id, name, slug, owner_id, status, plan,
                           max_instances, max_users, max_storage_gb,
                           created_at, updated_at, deleted_at
                    FROM organisations
                    WHERE id = $1
                    "#,
                    id.0
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
                    OrganisationRow,
                    r#"
                    SELECT id, name, slug, owner_id, status, plan,
                           max_instances, max_users, max_storage_gb,
                           created_at, updated_at, deleted_at
                    FROM organisations
                    WHERE id = $1
                    "#,
                    id.0
                )
                .fetch_optional(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to find organisation by id: {}", e),
        })?;

        row.map(|r| r.into_organisation()).transpose()
    }

    async fn find_by_slug(
        &self,
        slug: &OrganisationSlug,
    ) -> Result<Option<Organisation>, CoreError> {
        let row = match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query_as!(
                    OrganisationRow,
                    r#"
                    SELECT id, name, slug, owner_id, status, plan,
                           max_instances, max_users, max_storage_gb,
                           created_at, updated_at, deleted_at
                    FROM organisations
                    WHERE slug = $1
                    "#,
                    slug.as_str()
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
                    OrganisationRow,
                    r#"
                    SELECT id, name, slug, owner_id, status, plan,
                           max_instances, max_users, max_storage_gb,
                           created_at, updated_at, deleted_at
                    FROM organisations
                    WHERE slug = $1
                    "#,
                    slug.as_str()
                )
                .fetch_optional(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to find organisation by slug: {}", e),
        })?;

        row.map(|r| r.into_organisation()).transpose()
    }

    async fn find_by_owner(&self, owner_id: &UserId) -> Result<Vec<Organisation>, CoreError> {
        let rows = match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query_as!(
                    OrganisationRow,
                    r#"
                    SELECT id, name, slug, owner_id, status, plan,
                           max_instances, max_users, max_storage_gb,
                           created_at, updated_at, deleted_at
                    FROM organisations
                    WHERE owner_id = $1
                    ORDER BY created_at DESC
                    "#,
                    owner_id.0
                )
                .fetch_all(*pool)
                .await
            }
            PgExecutor::Tx(tx) => {
                let mut guard = tx.lock().await;
                let transaction = guard
                    .as_mut()
                    .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))?;
                sqlx::query_as!(
                    OrganisationRow,
                    r#"
                    SELECT id, name, slug, owner_id, status, plan,
                           max_instances, max_users, max_storage_gb,
                           created_at, updated_at, deleted_at
                    FROM organisations
                    WHERE owner_id = $1
                    ORDER BY created_at DESC
                    "#,
                    owner_id.0
                )
                .fetch_all(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to find organisations by owner: {}", e),
        })?;

        rows.into_iter().map(|r| r.into_organisation()).collect()
    }

    async fn find_by_member(&self, member_id: &UserId) -> Result<Vec<Organisation>, CoreError> {
        // TODO: Implement this once members table is created
        // This requires a separate table to track organisation memberships:
        //
        // CREATE TABLE members (
        //     organisation_id UUID NOT NULL REFERENCES organisations(id),
        //     user_id UUID NOT NULL,
        //     role VARCHAR(50) NOT NULL,
        //     created_at TIMESTAMPTZ NOT NULL,
        //     PRIMARY KEY (organisation_id, user_id)
        // );
        //
        // Then use this query:
        // SELECT o.* FROM organisations o
        // INNER JOIN members m ON o.id = m.organisation_id
        // WHERE m.user_id = $1
        // ORDER BY o.created_at DESC

        let organisations = match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query_as!(
                    OrganisationRow,
                    r#"
                    SELECT o.id, o.name, o.slug, o.owner_id, o.status, o.plan,
                           o.max_instances, o.max_users, o.max_storage_gb,
                           o.created_at, o.updated_at, o.deleted_at
                    FROM organisations o
                    INNER JOIN members m ON o.id = m.organisation_id
                    WHERE m.user_id = $1
                    ORDER BY o.created_at DESC
                    "#,
                    member_id.0
                )
                .fetch_all(*pool)
                .await
            }
            PgExecutor::Tx(tx) => {
                let mut guard = tx.lock().await;
                let transaction = guard
                    .as_mut()
                    .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))?;
                sqlx::query_as!(
                    OrganisationRow,
                    r#"
                    SELECT o.id, o.name, o.slug, o.owner_id, o.status, o.plan,
                           o.max_instances, o.max_users, o.max_storage_gb,
                           o.created_at, o.updated_at, o.deleted_at
                    FROM organisations o
                    INNER JOIN members m ON o.id = m.organisation_id
                    WHERE m.user_id = $1
                    ORDER BY o.created_at DESC
                    "#,
                    member_id.0
                )
                .fetch_all(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to find organisations by member: {}", e),
        })?;

        let organisations = organisations
            .into_iter()
            .map(|r| r.into_organisation())
            .collect::<Result<Vec<Organisation>, CoreError>>()?;

        Ok(organisations)
    }

    async fn list(
        &self,
        status: Option<OrganisationStatus>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Organisation>, CoreError> {
        let status_str = status.as_ref().map(|s| s.to_string());

        let rows = match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query_as!(
                    OrganisationRow,
                    r#"
                    SELECT id, name, slug, owner_id, status, plan,
                           max_instances, max_users, max_storage_gb,
                           created_at, updated_at, deleted_at
                    FROM organisations
                    WHERE ($1::text IS NULL OR status = $1)
                    ORDER BY created_at DESC
                    LIMIT $2 OFFSET $3
                    "#,
                    status_str.as_deref(),
                    limit as i64,
                    offset as i64
                )
                .fetch_all(*pool)
                .await
            }
            PgExecutor::Tx(tx) => {
                let mut guard = tx.lock().await;
                let transaction = guard
                    .as_mut()
                    .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))?;
                sqlx::query_as!(
                    OrganisationRow,
                    r#"
                    SELECT id, name, slug, owner_id, status, plan,
                           max_instances, max_users, max_storage_gb,
                           created_at, updated_at, deleted_at
                    FROM organisations
                    WHERE ($1::text IS NULL OR status = $1)
                    ORDER BY created_at DESC
                    LIMIT $2 OFFSET $3
                    "#,
                    status_str.as_deref(),
                    limit as i64,
                    offset as i64
                )
                .fetch_all(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to list organisations: {}", e),
        })?;

        rows.into_iter().map(|r| r.into_organisation()).collect()
    }

    async fn update(&self, organisation: Organisation) -> Result<Organisation, CoreError> {
        let now = Utc::now();

        match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query!(
                    r#"
                    UPDATE organisations
                    SET name = $2,
                        slug = $3,
                        status = $4,
                        plan = $5,
                        max_instances = $6,
                        max_users = $7,
                        max_storage_gb = $8,
                        updated_at = $9,
                        deleted_at = $10
                    WHERE id = $1
                    "#,
                    organisation.id.0,
                    organisation.name.as_str(),
                    organisation.slug.as_str(),
                    organisation.status.to_string(),
                    organisation.plan.to_string(),
                    organisation.limits.max_instances as i32,
                    organisation.limits.max_users as i32,
                    organisation.limits.max_storage_gb as i32,
                    now,
                    organisation.deleted_at,
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
                    UPDATE organisations
                    SET name = $2,
                        slug = $3,
                        status = $4,
                        plan = $5,
                        max_instances = $6,
                        max_users = $7,
                        max_storage_gb = $8,
                        updated_at = $9,
                        deleted_at = $10
                    WHERE id = $1
                    "#,
                    organisation.id.0,
                    organisation.name.as_str(),
                    organisation.slug.as_str(),
                    organisation.status.to_string(),
                    organisation.plan.to_string(),
                    organisation.limits.max_instances as i32,
                    organisation.limits.max_users as i32,
                    organisation.limits.max_storage_gb as i32,
                    now,
                    organisation.deleted_at,
                )
                .execute(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to update organisation: {}", e),
        })?;

        Ok(Organisation {
            updated_at: now,
            ..organisation
        })
    }

    async fn delete(&self, id: &OrganisationId) -> Result<(), CoreError> {
        let now = Utc::now();

        match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query!(
                    r#"
                    UPDATE organisations
                    SET status = $2,
                        deleted_at = $3,
                        updated_at = $3
                    WHERE id = $1
                    "#,
                    id.0,
                    OrganisationStatus::Deleted.to_string(),
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
                    UPDATE organisations
                    SET status = $2,
                        deleted_at = $3,
                        updated_at = $3
                    WHERE id = $1
                    "#,
                    id.0,
                    OrganisationStatus::Deleted.to_string(),
                    now,
                )
                .execute(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to delete organisation: {}", e),
        })?;

        Ok(())
    }

    async fn slug_exists(&self, slug: &OrganisationSlug) -> Result<bool, CoreError> {
        let exists = match &self.executor {
            PgExecutor::Pool(pool) => sqlx::query!(
                r#"
                    SELECT EXISTS(SELECT 1 FROM organisations WHERE slug = $1) as "exists!"
                    "#,
                slug.as_str()
            )
            .fetch_one(*pool)
            .await
            .map(|row| row.exists),
            PgExecutor::Tx(tx) => {
                let mut guard = tx.lock().await;
                let transaction = guard
                    .as_mut()
                    .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))?;
                sqlx::query!(
                    r#"
                    SELECT EXISTS(SELECT 1 FROM organisations WHERE slug = $1) as "exists!"
                    "#,
                    slug.as_str()
                )
                .fetch_one(transaction.as_mut())
                .await
                .map(|row| row.exists)
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to check slug existence: {}", e),
        })?;

        Ok(exists)
    }

    async fn count(&self) -> Result<usize, CoreError> {
        let count = match &self.executor {
            PgExecutor::Pool(pool) => sqlx::query!(
                r#"
                    SELECT COUNT(*) as "count!" FROM organisations
                    "#
            )
            .fetch_one(*pool)
            .await
            .map(|row| row.count),
            PgExecutor::Tx(tx) => {
                let mut guard = tx.lock().await;
                let transaction = guard
                    .as_mut()
                    .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))?;
                sqlx::query!(
                    r#"
                    SELECT COUNT(*) as "count!" FROM organisations
                    "#
                )
                .fetch_one(transaction.as_mut())
                .await
                .map(|row| row.count)
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to count organisations: {}", e),
        })?;

        Ok(count as usize)
    }

    async fn count_by_status(&self, status: OrganisationStatus) -> Result<usize, CoreError> {
        let count = match &self.executor {
            PgExecutor::Pool(pool) => sqlx::query!(
                r#"
                    SELECT COUNT(*) as "count!" FROM organisations WHERE status = $1
                    "#,
                status.to_string()
            )
            .fetch_one(*pool)
            .await
            .map(|row| row.count),
            PgExecutor::Tx(tx) => {
                let mut guard = tx.lock().await;
                let transaction = guard
                    .as_mut()
                    .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))?;
                sqlx::query!(
                    r#"
                    SELECT COUNT(*) as "count!" FROM organisations WHERE status = $1
                    "#,
                    status.to_string()
                )
                .fetch_one(transaction.as_mut())
                .await
                .map(|row| row.count)
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to count organisations by status: {}", e),
        })?;

        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn sample_time() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn sample_row() -> OrganisationRow {
        OrganisationRow {
            id: Uuid::parse_str("dddddddd-dddd-dddd-dddd-dddddddddddd").unwrap(),
            name: "Acme Cloud".to_string(),
            slug: "acme-cloud".to_string(),
            owner_id: Uuid::parse_str("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee").unwrap(),
            status: "active".to_string(),
            plan: "starter".to_string(),
            max_instances: 5,
            max_users: 10,
            max_storage_gb: 20,
            created_at: sample_time(),
            updated_at: sample_time(),
            deleted_at: None,
        }
    }

    #[test]
    fn organisation_row_into_organisation_maps_fields() {
        let row = sample_row();
        let organisation = row.into_organisation().unwrap();

        assert_eq!(
            organisation.id.0,
            Uuid::parse_str("dddddddd-dddd-dddd-dddd-dddddddddddd").unwrap()
        );
        assert_eq!(organisation.name.as_str(), "Acme Cloud");
        assert_eq!(organisation.slug.as_str(), "acme-cloud");
        assert_eq!(
            organisation.owner_id.0,
            Uuid::parse_str("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee").unwrap()
        );
        assert_eq!(organisation.status, OrganisationStatus::Active);
        assert_eq!(organisation.plan.to_string(), "starter");
        assert_eq!(organisation.limits.max_instances, 5);
        assert_eq!(organisation.limits.max_users, 10);
        assert_eq!(organisation.limits.max_storage_gb, 20);
        assert_eq!(organisation.created_at, sample_time());
        assert_eq!(organisation.updated_at, sample_time());
        assert!(organisation.deleted_at.is_none());
    }

    #[test]
    fn organisation_row_into_organisation_rejects_invalid_name() {
        let mut row = sample_row();
        row.name = "ab".to_string();

        let err = row.into_organisation().unwrap_err();
        assert!(matches!(err, CoreError::InvalidOrganisationName { .. }));
    }

    #[test]
    fn organisation_row_into_organisation_rejects_invalid_slug() {
        let mut row = sample_row();
        row.slug = "bad_slug".to_string();

        let err = row.into_organisation().unwrap_err();
        assert!(matches!(err, CoreError::InvalidOrganisationSlug { .. }));
    }
}
