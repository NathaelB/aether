use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::domain::{
    CoreError,
    organisation::OrganisationId,
    role::{Role, RoleId, ports::RoleRepository},
};

#[derive(FromRow)]
struct RoleRow {
    id: Uuid,
    name: String,
    permissions: i64,
    organisation_id: Option<Uuid>,
    color: Option<String>,
    created_at: DateTime<Utc>,
}

impl RoleRow {
    fn into_role(self) -> Role {
        Role {
            id: RoleId(self.id),
            name: self.name,
            permissions: self.permissions as u64,
            organisation_id: self.organisation_id.map(OrganisationId),
            color: self.color,
            created_at: self.created_at,
        }
    }
}

#[derive(Clone)]
pub struct PostgresRoleRepository {
    pool: PgPool,
}

impl PostgresRoleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl RoleRepository for PostgresRoleRepository {
    async fn insert(&self, role: Role) -> Result<(), CoreError> {
        sqlx::query!(
            r#"
            INSERT INTO roles (
                id, name, permissions, organisation_id, color, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            role.id.0,
            role.name,
            role.permissions as i64,
            role.organisation_id.map(|id| id.0),
            role.color,
            role.created_at,
            Utc::now(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to insert role: {}", e),
        })?;

        Ok(())
    }

    async fn get_by_id(&self, role_id: RoleId) -> Result<Option<Role>, CoreError> {
        let row = sqlx::query_as!(
            RoleRow,
            r#"
            SELECT id, name, permissions, organisation_id, color, created_at
            FROM roles
            WHERE id = $1
            "#,
            role_id.0
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to get role by id: {}", e),
        })?;

        Ok(row.map(RoleRow::into_role))
    }

    async fn list_by_organisation(
        &self,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Role>, CoreError> {
        let rows = sqlx::query_as!(
            RoleRow,
            r#"
            SELECT id, name, permissions, organisation_id, color, created_at
            FROM roles
            WHERE organisation_id = $1
            ORDER BY created_at DESC
            "#,
            organisation_id.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to list roles by organisation: {}", e),
        })?;

        Ok(rows.into_iter().map(RoleRow::into_role).collect())
    }

    async fn list_by_names(
        &self,
        organisation_id: OrganisationId,
        names: Vec<String>,
    ) -> Result<Vec<Role>, CoreError> {
        if names.is_empty() {
            return Ok(vec![]);
        }

        let rows = sqlx::query_as!(
            RoleRow,
            r#"
            SELECT id, name, permissions, organisation_id, color, created_at
            FROM roles
            WHERE organisation_id = $1
              AND name = ANY($2)
            ORDER BY created_at DESC
            "#,
            organisation_id.0,
            &names
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to list roles by names: {}", e),
        })?;

        Ok(rows.into_iter().map(RoleRow::into_role).collect())
    }

    async fn update(&self, role: Role) -> Result<(), CoreError> {
        sqlx::query!(
            r#"
            UPDATE roles
            SET name = $2,
                permissions = $3,
                organisation_id = $4,
                color = $5,
                updated_at = $6
            WHERE id = $1
            "#,
            role.id.0,
            role.name,
            role.permissions as i64,
            role.organisation_id.map(|id| id.0),
            role.color,
            Utc::now(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to update role: {}", e),
        })?;

        Ok(())
    }

    async fn delete(&self, role_id: RoleId) -> Result<(), CoreError> {
        sqlx::query!(
            r#"
            DELETE FROM roles
            WHERE id = $1
            "#,
            role_id.0
        )
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to delete role: {}", e),
        })?;

        Ok(())
    }
}
