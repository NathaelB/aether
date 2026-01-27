use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use aether_domain::{
    CoreError,
    deployments::{
        Deployment, DeploymentId, DeploymentKind, DeploymentName, DeploymentStatus,
        DeploymentVersion, ports::DeploymentRepository,
    },
    organisation::OrganisationId,
    user::UserId,
};
use aether_persistence::{PgExecutor, PgTransaction};

#[derive(FromRow)]
struct DeploymentRow {
    id: Uuid,
    organisation_id: Uuid,
    name: String,
    kind: String,
    status: String,
    namespace: String,
    version: Option<String>,
    created_by: Uuid,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deployed_at: Option<DateTime<Utc>>,
    deleted_at: Option<DateTime<Utc>>,
}

impl DeploymentRow {
    fn into_deployment(self) -> Result<Deployment, CoreError> {
        let kind = DeploymentKind::try_from(self.kind.as_str())?;
        let status = DeploymentStatus::try_from(self.status.as_str())?;
        let version = self.version.unwrap_or_default();

        Ok(Deployment {
            id: DeploymentId(self.id),
            organisation_id: OrganisationId(self.organisation_id),
            name: DeploymentName(self.name),
            kind,
            version: DeploymentVersion(version),
            status,
            namespace: self.namespace,
            created_by: UserId(self.created_by),
            created_at: self.created_at,
            updated_at: self.updated_at,
            deployed_at: self.deployed_at,
            deleted_at: self.deleted_at,
        })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub struct PostgresDeploymentRepository<'e, 't> {
    executor: PgExecutor<'e, 't>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'e, 't> PostgresDeploymentRepository<'e, 't> {
    pub fn new(executor: PgExecutor<'e, 't>) -> Self {
        Self { executor }
    }

    pub fn from_tx(tx: &'e PgTransaction<'t>) -> Self {
        Self::new(PgExecutor::from_tx(tx))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'e> PostgresDeploymentRepository<'e, 'e> {
    pub fn from_pool(pool: &'e sqlx::PgPool) -> Self {
        Self::new(PgExecutor::from_pool(pool))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl DeploymentRepository for PostgresDeploymentRepository<'_, '_> {
    async fn insert(&self, deployment: Deployment) -> Result<(), CoreError> {
        match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query!(
                    r#"
                    INSERT INTO deployments (
                        id,
                        organisation_id,
                        name,
                        kind,
                        status,
                        namespace,
                        version,
                        created_by,
                        created_at,
                        updated_at,
                        deployed_at,
                        deleted_at
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                    "#,
                    deployment.id.0,
                    deployment.organisation_id.0,
                    deployment.name.0,
                    deployment.kind.to_string(),
                    deployment.status.to_string(),
                    deployment.namespace,
                    deployment.version.0,
                    deployment.created_by.0,
                    deployment.created_at,
                    deployment.updated_at,
                    deployment.deployed_at,
                    deployment.deleted_at,
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
                    INSERT INTO deployments (
                        id,
                        organisation_id,
                        name,
                        kind,
                        status,
                        namespace,
                        version,
                        created_by,
                        created_at,
                        updated_at,
                        deployed_at,
                        deleted_at
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                    "#,
                    deployment.id.0,
                    deployment.organisation_id.0,
                    deployment.name.0,
                    deployment.kind.to_string(),
                    deployment.status.to_string(),
                    deployment.namespace,
                    deployment.version.0,
                    deployment.created_by.0,
                    deployment.created_at,
                    deployment.updated_at,
                    deployment.deployed_at,
                    deployment.deleted_at,
                )
                .execute(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to insert deployment: {}", e),
        })?;

        Ok(())
    }

    async fn get_by_id(
        &self,
        deployment_id: DeploymentId,
    ) -> Result<Option<Deployment>, CoreError> {
        let row = match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query_as!(
                    DeploymentRow,
                    r#"
                    SELECT id,
                           organisation_id,
                           name,
                           kind,
                           status,
                           namespace,
                           version,
                           created_by,
                           created_at,
                           updated_at,
                           deployed_at,
                           deleted_at
                    FROM deployments
                    WHERE id = $1
                    "#,
                    deployment_id.0
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
                    DeploymentRow,
                    r#"
                    SELECT id,
                           organisation_id,
                           name,
                           kind,
                           status,
                           namespace,
                           version,
                           created_by,
                           created_at,
                           updated_at,
                           deployed_at,
                           deleted_at
                    FROM deployments
                    WHERE id = $1
                    "#,
                    deployment_id.0
                )
                .fetch_optional(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to get deployment by id: {}", e),
        })?;

        row.map(|r| r.into_deployment()).transpose()
    }

    async fn list_by_organisation(
        &self,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Deployment>, CoreError> {
        let rows = match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query_as!(
                    DeploymentRow,
                    r#"
                    SELECT id,
                           organisation_id,
                           name,
                           kind,
                           status,
                           namespace,
                           version,
                           created_by,
                           created_at,
                           updated_at,
                           deployed_at,
                           deleted_at
                    FROM deployments
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
                    DeploymentRow,
                    r#"
                    SELECT id,
                           organisation_id,
                           name,
                           kind,
                           status,
                           namespace,
                           version,
                           created_by,
                           created_at,
                           updated_at,
                           deployed_at,
                           deleted_at
                    FROM deployments
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
            message: format!("Failed to list deployments by organisation: {}", e),
        })?;

        rows.into_iter().map(|r| r.into_deployment()).collect()
    }

    async fn update(&self, deployment: Deployment) -> Result<(), CoreError> {
        match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query!(
                    r#"
                    UPDATE deployments
                    SET name = $2,
                        kind = $3,
                        status = $4,
                        namespace = $5,
                        version = $6,
                        updated_at = $7,
                        deployed_at = $8,
                        deleted_at = $9
                    WHERE id = $1
                    "#,
                    deployment.id.0,
                    deployment.name.0,
                    deployment.kind.to_string(),
                    deployment.status.to_string(),
                    deployment.namespace,
                    deployment.version.0,
                    deployment.updated_at,
                    deployment.deployed_at,
                    deployment.deleted_at,
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
                    UPDATE deployments
                    SET name = $2,
                        kind = $3,
                        status = $4,
                        namespace = $5,
                        version = $6,
                        updated_at = $7,
                        deployed_at = $8,
                        deleted_at = $9
                    WHERE id = $1
                    "#,
                    deployment.id.0,
                    deployment.name.0,
                    deployment.kind.to_string(),
                    deployment.status.to_string(),
                    deployment.namespace,
                    deployment.version.0,
                    deployment.updated_at,
                    deployment.deployed_at,
                    deployment.deleted_at,
                )
                .execute(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to update deployment: {}", e),
        })?;

        Ok(())
    }

    async fn delete(&self, deployment_id: DeploymentId) -> Result<(), CoreError> {
        match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query!(
                    r#"
                    UPDATE deployments
                    SET deleted_at = $2,
                        updated_at = $2
                    WHERE id = $1
                    "#,
                    deployment_id.0,
                    Utc::now()
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
                    UPDATE deployments
                    SET deleted_at = $2,
                        updated_at = $2
                    WHERE id = $1
                    "#,
                    deployment_id.0,
                    Utc::now()
                )
                .execute(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to delete deployment: {}", e),
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

    fn sample_row() -> DeploymentRow {
        DeploymentRow {
            id: Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap(),
            organisation_id: Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap(),
            name: "alpha".to_string(),
            kind: "ferris_key".to_string(),
            status: "successful".to_string(),
            namespace: "ns-alpha".to_string(),
            version: Some("1.2.3".to_string()),
            created_by: Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap(),
            created_at: sample_time(),
            updated_at: sample_time(),
            deployed_at: Some(sample_time()),
            deleted_at: None,
        }
    }

    #[test]
    fn deployment_row_into_deployment_maps_fields() {
        let row = sample_row();
        let deployment = row.into_deployment().unwrap();

        assert_eq!(
            deployment.id.0,
            Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap()
        );
        assert_eq!(
            deployment.organisation_id.0,
            Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap()
        );
        assert_eq!(deployment.name.0, "alpha");
        assert_eq!(deployment.kind, DeploymentKind::FerrisKey);
        assert_eq!(deployment.status, DeploymentStatus::Successful);
        assert_eq!(deployment.namespace, "ns-alpha");
        assert_eq!(deployment.version.0, "1.2.3");
        assert_eq!(
            deployment.created_by.0,
            Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap()
        );
        assert_eq!(deployment.created_at, sample_time());
        assert_eq!(deployment.updated_at, sample_time());
        assert_eq!(deployment.deployed_at, Some(sample_time()));
        assert!(deployment.deleted_at.is_none());
    }

    #[test]
    fn deployment_row_into_deployment_rejects_invalid_kind() {
        let mut row = sample_row();
        row.kind = "bogus".to_string();

        let err = row.into_deployment().unwrap_err();
        assert!(
            matches!(err, CoreError::InternalError(message) if message.contains("Invalid deployment kind"))
        );
    }

    #[test]
    fn deployment_row_into_deployment_rejects_invalid_status() {
        let mut row = sample_row();
        row.status = "unknown".to_string();

        let err = row.into_deployment().unwrap_err();
        assert!(
            matches!(err, CoreError::InternalError(message) if message.contains("Invalid deployment status"))
        );
    }
}
