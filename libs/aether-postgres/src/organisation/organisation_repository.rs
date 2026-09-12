use chrono::{DateTime, Utc};
use sqlx::FromRow;
use tracing::info;
use uuid::Uuid;

use aether_domain::{
    CoreError,
    organisation::{
        Organisation, OrganisationId,
        commands::CreateOrganisationData,
        member::{Member, MemberId},
        ports::OrganisationRepository,
        value_objects::{
            OrganisationLimits, OrganisationName, OrganisationSlug, OrganisationStatus,
        },
    },
    role::{Role, RoleId},
    user::UserId,
};
use aether_macros::repository;
use aether_persistence::SharedTx;

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
#[repository(domain = Organisation, backend = Postgres)]
pub struct PostgresOrganisationRepository<'tx> {
    tx: SharedTx<'tx>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'tx> PostgresOrganisationRepository<'tx> {
    pub fn new(tx: &SharedTx<'tx>) -> Self {
        Self { tx: tx.clone() }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl OrganisationRepository for PostgresOrganisationRepository<'_> {
    async fn create(&self, data: CreateOrganisationData) -> Result<Organisation, CoreError> {
        let id = OrganisationId::new();
        let now = Utc::now();
        let status = OrganisationStatus::Active;

        info!("Creating organisation with id: {}", id.0);

        {
            let mut tx = self.tx.lock().await;
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
            .execute(&mut ***tx)
            .await
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
        {
            let mut tx = self.tx.lock().await;
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
            .execute(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to insert organisation member: {}", e),
        })?;

        Ok(())
    }

    async fn find_member(
        &self,
        organisation_id: &OrganisationId,
        user_id: &UserId,
    ) -> Result<Option<Member>, CoreError> {
        // A left join, so a member holding no role comes back as a member
        // rather than as nobody. An inner join would make "in the
        // organisation and granted nothing yet" indistinguishable from "not
        // in it", and those are refused differently.
        let rows = {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            SELECT m.id            AS "member_id!",
                   u.email         AS "email!",
                   u.name          AS "name!",
                   m.created_at    AS "joined_at!",
                   m.invited_by,
                   r.id            AS "role_id?",
                   r.name          AS "role_name?",
                   r.permissions   AS "role_permissions?",
                   r.color         AS "role_color?",
                   r.created_at    AS "role_created_at?"
            FROM members m
            JOIN users u ON u.id = m.user_id
            LEFT JOIN member_roles mr ON mr.member_id = m.id
            LEFT JOIN roles r ON r.id = mr.role_id
            WHERE m.organisation_id = $1 AND m.user_id = $2
            "#,
                organisation_id.0,
                user_id.0,
            )
            .fetch_all(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to read organisation membership: {}", e),
        })?;

        let Some(first) = rows.first() else {
            return Ok(None);
        };

        let roles = rows
            .iter()
            .filter_map(|row| {
                Some(Role {
                    id: RoleId(row.role_id?),
                    name: row.role_name.clone()?,
                    permissions: row.role_permissions? as u64,
                    organisation_id: Some(*organisation_id),
                    color: row.role_color.clone(),
                    created_at: row.role_created_at?,
                })
            })
            .collect();

        Ok(Some(Member {
            id: MemberId(first.member_id),
            organisation_id: *organisation_id,
            user_id: *user_id,
            email: first.email.clone(),
            name: first.name.clone(),
            roles,
            joined_at: first.joined_at,
            invited_by: first.invited_by.map(UserId),
        }))
    }

    async fn list_members(
        &self,
        organisation_id: &OrganisationId,
    ) -> Result<Vec<Member>, CoreError> {
        // Ordered so the list does not shuffle between two reads: the roles
        // of one member have to arrive together for the fold below, and a
        // screen showing them in a different order each time reads as change.
        let rows = {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            SELECT m.id            AS "member_id!",
                   m.user_id       AS "user_id!",
                   u.email         AS "email!",
                   u.name          AS "name!",
                   m.created_at    AS "joined_at!",
                   m.invited_by,
                   r.id            AS "role_id?",
                   r.name          AS "role_name?",
                   r.permissions   AS "role_permissions?",
                   r.color         AS "role_color?",
                   r.created_at    AS "role_created_at?"
            FROM members m
            JOIN users u ON u.id = m.user_id
            LEFT JOIN member_roles mr ON mr.member_id = m.id
            LEFT JOIN roles r ON r.id = mr.role_id
            WHERE m.organisation_id = $1
            ORDER BY m.created_at, m.id, r.name
            "#,
                organisation_id.0,
            )
            .fetch_all(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to list organisation members: {}", e),
        })?;

        let mut members: Vec<Member> = Vec::new();

        for row in rows {
            let role = row.role_id.map(|id| Role {
                id: RoleId(id),
                name: row.role_name.clone().unwrap_or_default(),
                permissions: row.role_permissions.unwrap_or_default() as u64,
                organisation_id: Some(*organisation_id),
                color: row.role_color.clone(),
                created_at: row.role_created_at.unwrap_or(row.joined_at),
            });

            match members.last_mut() {
                Some(last) if last.id.0 == row.member_id => {
                    last.roles.extend(role);
                }
                _ => members.push(Member {
                    id: MemberId(row.member_id),
                    organisation_id: *organisation_id,
                    user_id: UserId(row.user_id),
                    email: row.email.clone(),
                    name: row.name.clone(),
                    roles: role.into_iter().collect(),
                    joined_at: row.joined_at,
                    invited_by: row.invited_by.map(UserId),
                }),
            }
        }

        Ok(members)
    }

    async fn set_member_roles(
        &self,
        organisation_id: &OrganisationId,
        user_id: &UserId,
        roles: &[RoleId],
    ) -> Result<(), CoreError> {
        let ids: Vec<Uuid> = roles.iter().map(|role| role.0).collect();
        let mut tx = self.tx.lock().await;

        // Cleared then written, in one transaction, because this replaces a
        // set rather than adding to one. Adding what is missing and removing
        // what is extra would be the same result reached in a way that has to
        // be got right twice.
        sqlx::query!(
            r#"
        DELETE FROM member_roles
        WHERE member_id IN (
            SELECT id FROM members WHERE organisation_id = $1 AND user_id = $2
        )
        "#,
            organisation_id.0,
            user_id.0,
        )
        .execute(&mut ***tx)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to clear member roles: {}", e),
        })?;

        if ids.is_empty() {
            return Ok(());
        }

        sqlx::query!(
            r#"
        INSERT INTO member_roles (member_id, role_id)
        SELECT m.id, r.id
        FROM members m
        JOIN roles r ON r.id = ANY($3) AND r.organisation_id = $1
        WHERE m.organisation_id = $1 AND m.user_id = $2
        "#,
            organisation_id.0,
            user_id.0,
            &ids,
        )
        .execute(&mut ***tx)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to grant member roles: {}", e),
        })?;

        Ok(())
    }

    async fn remove_member(
        &self,
        organisation_id: &OrganisationId,
        user_id: &UserId,
    ) -> Result<(), CoreError> {
        {
            let mut tx = self.tx.lock().await;
            // The grants go with it, through the cascade on member_roles.
            sqlx::query!(
                r#"
            DELETE FROM members WHERE organisation_id = $1 AND user_id = $2
            "#,
                organisation_id.0,
                user_id.0,
            )
            .execute(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to remove organisation member: {}", e),
        })?;

        Ok(())
    }

    async fn find_by_id(&self, id: &OrganisationId) -> Result<Option<Organisation>, CoreError> {
        let row = {
            let mut tx = self.tx.lock().await;
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
            .fetch_optional(&mut ***tx)
            .await
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
        let row = {
            let mut tx = self.tx.lock().await;
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
            .fetch_optional(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to find organisation by slug: {}", e),
        })?;

        row.map(|r| r.into_organisation()).transpose()
    }

    async fn find_by_owner(&self, owner_id: &UserId) -> Result<Vec<Organisation>, CoreError> {
        let rows = {
            let mut tx = self.tx.lock().await;
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
            .fetch_all(&mut ***tx)
            .await
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

        let organisations = {
            let mut tx = self.tx.lock().await;
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
            .fetch_all(&mut ***tx)
            .await
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

        let rows = {
            let mut tx = self.tx.lock().await;
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
            .fetch_all(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to list organisations: {}", e),
        })?;

        rows.into_iter().map(|r| r.into_organisation()).collect()
    }

    async fn update(&self, organisation: Organisation) -> Result<Organisation, CoreError> {
        let now = Utc::now();

        {
            let mut tx = self.tx.lock().await;
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
            .execute(&mut ***tx)
            .await
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

        {
            let mut tx = self.tx.lock().await;
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
            .execute(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to delete organisation: {}", e),
        })?;

        Ok(())
    }

    async fn slug_exists(&self, slug: &OrganisationSlug) -> Result<bool, CoreError> {
        let exists = {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            SELECT EXISTS(SELECT 1 FROM organisations WHERE slug = $1) as "exists!"
            "#,
                slug.as_str()
            )
            .fetch_one(&mut ***tx)
            .await
            .map(|row| row.exists)
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to check slug existence: {}", e),
        })?;

        Ok(exists)
    }

    async fn count(&self) -> Result<usize, CoreError> {
        let count = {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            SELECT COUNT(*) as "count!" FROM organisations
            "#
            )
            .fetch_one(&mut ***tx)
            .await
            .map(|row| row.count)
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to count organisations: {}", e),
        })?;

        Ok(count as usize)
    }

    async fn count_by_status(&self, status: OrganisationStatus) -> Result<usize, CoreError> {
        let count = {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            SELECT COUNT(*) as "count!" FROM organisations WHERE status = $1
            "#,
                status.to_string()
            )
            .fetch_one(&mut ***tx)
            .await
            .map(|row| row.count)
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
