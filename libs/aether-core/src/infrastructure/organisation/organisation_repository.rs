use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::domain::{
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
#[derive(Clone)]
pub struct PostgresOrganisationRepository {
    pool: PgPool,
}

impl PostgresOrganisationRepository {
    /// Creates a new PostgresOrganisationRepository
    ///
    /// # Arguments
    /// * `pool` - A connection pool to the PostgreSQL database
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl OrganisationRepository for PostgresOrganisationRepository {
    async fn create(&self, data: CreateOrganisationData) -> Result<Organisation, CoreError> {
        let id = OrganisationId::new();
        let now = Utc::now();
        let status = OrganisationStatus::Active;

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
        .execute(&self.pool)
        .await
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
        sqlx::query!(
            r#"
            INSERT INTO members (organisation_id, user_id, created_at)
            VALUES ($1, $2, $3)
            "#,
            organisation_id.0,
            user_id.0,
            Utc::now(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to insert organisation member: {}", e),
        })?;

        Ok(())
    }

    async fn find_by_id(&self, id: &OrganisationId) -> Result<Option<Organisation>, CoreError> {
        let row = sqlx::query_as!(
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
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to find organisation by id: {}", e),
        })?;

        row.map(|r| r.into_organisation()).transpose()
    }

    async fn find_by_slug(
        &self,
        slug: &OrganisationSlug,
    ) -> Result<Option<Organisation>, CoreError> {
        let row = sqlx::query_as!(
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
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to find organisation by slug: {}", e),
        })?;

        row.map(|r| r.into_organisation()).transpose()
    }

    async fn find_by_owner(&self, owner_id: &UserId) -> Result<Vec<Organisation>, CoreError> {
        let rows = sqlx::query_as!(
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
        .fetch_all(&self.pool)
        .await
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

        let organisations = sqlx::query_as!(
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
        .fetch_all(&self.pool)
        .await
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

        let rows = sqlx::query_as!(
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
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to list organisations: {}", e),
        })?;

        rows.into_iter().map(|r| r.into_organisation()).collect()
    }

    async fn update(&self, organisation: Organisation) -> Result<Organisation, CoreError> {
        let now = Utc::now();

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
        .execute(&self.pool)
        .await
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
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to delete organisation: {}", e),
        })?;

        Ok(())
    }

    async fn slug_exists(&self, slug: &OrganisationSlug) -> Result<bool, CoreError> {
        let result = sqlx::query!(
            r#"
            SELECT EXISTS(SELECT 1 FROM organisations WHERE slug = $1) as "exists!"
            "#,
            slug.as_str()
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to check slug existence: {}", e),
        })?;

        Ok(result.exists)
    }

    async fn count(&self) -> Result<usize, CoreError> {
        let result = sqlx::query!(
            r#"
            SELECT COUNT(*) as "count!" FROM organisations
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to count organisations: {}", e),
        })?;

        Ok(result.count as usize)
    }

    async fn count_by_status(&self, status: OrganisationStatus) -> Result<usize, CoreError> {
        let result = sqlx::query!(
            r#"
            SELECT COUNT(*) as "count!" FROM organisations WHERE status = $1
            "#,
            status.to_string()
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to count organisations by status: {}", e),
        })?;

        Ok(result.count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::organisation::value_objects::Plan;

    // Note: These tests require a running PostgreSQL database
    // They are integration tests and should be run with:
    // cargo test --features postgres -- --test-threads=1

    async fn setup_test_db() -> PgPool {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/aether_test".to_string());

        PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to test database")
    }

    #[tokio::test]
    #[ignore] // Run only when DATABASE_URL is set
    async fn test_create_organisation() {
        let pool = setup_test_db().await;
        let repo = PostgresOrganisationRepository::new(pool);

        let name = OrganisationName::new("Test Org").unwrap();
        let slug = OrganisationSlug::new("test-org").unwrap();
        let owner_id = UserId(Uuid::new_v4());
        let plan = Plan::Starter;

        let data = CreateOrganisationData {
            name: name.clone(),
            slug: slug.clone(),
            owner_id,
            plan,
            limits: OrganisationLimits::from_plan(&plan),
        };

        let result = repo.create(data).await;
        assert!(result.is_ok());

        let org = result.unwrap();
        assert_eq!(org.name, name);
        assert_eq!(org.slug, slug);
        assert_eq!(org.owner_id, owner_id);
        assert_eq!(org.status, OrganisationStatus::Active);
    }

    #[tokio::test]
    #[ignore] // Run only when DATABASE_URL is set
    async fn test_slug_exists() {
        let pool = setup_test_db().await;
        let repo = PostgresOrganisationRepository::new(pool);

        let slug = OrganisationSlug::new("existing-slug").unwrap();

        // First check should return false
        let exists = repo.slug_exists(&slug).await.unwrap();
        assert!(!exists);

        // Create organisation with this slug
        let data = CreateOrganisationData {
            name: OrganisationName::new("Existing Org").unwrap(),
            slug: slug.clone(),
            owner_id: UserId(Uuid::new_v4()),
            plan: Plan::Free,
            limits: OrganisationLimits::from_plan(&Plan::Free),
        };
        repo.create(data).await.unwrap();

        // Now check should return true
        let exists = repo.slug_exists(&slug).await.unwrap();
        assert!(exists);
    }
}
