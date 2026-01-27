use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use aether_domain::{
    CoreError,
    organisation::OrganisationId,
    role::{Role, RoleId, ports::RoleRepository},
};
use aether_persistence::{PgExecutor, PgTransaction};

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

#[cfg_attr(coverage_nightly, coverage(off))]
pub struct PostgresRoleRepository<'e, 't> {
    executor: PgExecutor<'e, 't>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'e, 't> PostgresRoleRepository<'e, 't> {
    pub fn new(executor: PgExecutor<'e, 't>) -> Self {
        Self { executor }
    }

    pub fn from_tx(tx: &'e PgTransaction<'t>) -> Self {
        Self::new(PgExecutor::from_tx(tx))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'e> PostgresRoleRepository<'e, 'e> {
    pub fn from_pool(pool: &'e sqlx::PgPool) -> Self {
        Self::new(PgExecutor::from_pool(pool))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl RoleRepository for PostgresRoleRepository<'_, '_> {
    async fn insert(&self, role: Role) -> Result<(), CoreError> {
        match &self.executor {
            PgExecutor::Pool(pool) => {
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
                .execute(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to insert role: {}", e),
        })?;

        Ok(())
    }

    async fn get_by_id(&self, role_id: RoleId) -> Result<Option<Role>, CoreError> {
        let row = match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query_as!(
                    RoleRow,
                    r#"
                    SELECT id, name, permissions, organisation_id, color, created_at
                    FROM roles
                    WHERE id = $1
                    "#,
                    role_id.0
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
                    RoleRow,
                    r#"
                    SELECT id, name, permissions, organisation_id, color, created_at
                    FROM roles
                    WHERE id = $1
                    "#,
                    role_id.0
                )
                .fetch_optional(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to get role by id: {}", e),
        })?;

        Ok(row.map(RoleRow::into_role))
    }

    async fn list_by_organisation(
        &self,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Role>, CoreError> {
        let rows = match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query_as!(
                    RoleRow,
                    r#"
                    SELECT id, name, permissions, organisation_id, color, created_at
                    FROM roles
                    WHERE organisation_id = $1
                    ORDER BY created_at DESC
                    "#,
                    organisation_id.0
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
                    RoleRow,
                    r#"
                    SELECT id, name, permissions, organisation_id, color, created_at
                    FROM roles
                    WHERE organisation_id = $1
                    ORDER BY created_at DESC
                    "#,
                    organisation_id.0
                )
                .fetch_all(transaction.as_mut())
                .await
            }
        }
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

        let rows = match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query_as!(
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
                .fetch_all(*pool)
                .await
            }
            PgExecutor::Tx(tx) => {
                let mut guard = tx.lock().await;
                let transaction = guard
                    .as_mut()
                    .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))?;
                sqlx::query_as!(
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
                .fetch_all(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to list roles by names: {}", e),
        })?;

        Ok(rows.into_iter().map(RoleRow::into_role).collect())
    }

    async fn update(&self, role: Role) -> Result<(), CoreError> {
        match &self.executor {
            PgExecutor::Pool(pool) => {
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
                .execute(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to update role: {}", e),
        })?;

        Ok(())
    }

    async fn delete(&self, role_id: RoleId) -> Result<(), CoreError> {
        match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query!(
                    r#"
                    DELETE FROM roles
                    WHERE id = $1
                    "#,
                    role_id.0
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
                    DELETE FROM roles
                    WHERE id = $1
                    "#,
                    role_id.0
                )
                .execute(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to delete role: {}", e),
        })?;

        Ok(())
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

    #[test]
    fn role_row_into_role_maps_fields_with_org() {
        let row = RoleRow {
            id: Uuid::parse_str("ffffffff-ffff-ffff-ffff-ffffffffffff").unwrap(),
            name: "admin".to_string(),
            permissions: 42,
            organisation_id: Some(Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap()),
            color: Some("blue".to_string()),
            created_at: sample_time(),
        };

        let role = row.into_role();

        assert_eq!(
            role.id.0,
            Uuid::parse_str("ffffffff-ffff-ffff-ffff-ffffffffffff").unwrap()
        );
        assert_eq!(role.name, "admin");
        assert_eq!(role.permissions, 42);
        assert_eq!(
            role.organisation_id.unwrap().0,
            Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap()
        );
        assert_eq!(role.color.as_deref(), Some("blue"));
        assert_eq!(role.created_at, sample_time());
    }

    #[test]
    fn role_row_into_role_maps_fields_without_org() {
        let row = RoleRow {
            id: Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
            name: "viewer".to_string(),
            permissions: 0,
            organisation_id: None,
            color: None,
            created_at: sample_time(),
        };

        let role = row.into_role();

        assert_eq!(role.name, "viewer");
        assert_eq!(role.permissions, 0);
        assert!(role.organisation_id.is_none());
        assert!(role.color.is_none());
    }
}
