use std::num::NonZeroU64;

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use aether_domain::{
    CoreError,
    backups::{
        ArchivePrefix, Backup, BackupId, BackupMethod, PostgresMajor,
        keys::{KeyName, KeyRef, KeyVersion, ProviderName},
        ports::BackupRepository,
    },
    catalog::ReleaseId,
    deployments::{DeploymentId, DeploymentKind},
    organisation::OrganisationId,
    version::Version,
};
use aether_macros::repository;
use aether_persistence::SharedTx;

#[derive(FromRow)]
struct BackupRow {
    id: Uuid,
    deployment_id: Uuid,
    organisation_id: Uuid,
    kind: String,
    version: String,
    postgres_major: i32,
    method: String,
    key_provider: String,
    key_name: String,
    key_version: i32,
    object_key: String,
    size_bytes: i64,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
}

impl BackupRow {
    fn into_backup(self) -> Result<Backup, CoreError> {
        let organisation_id = OrganisationId(self.organisation_id);
        let deployment_id = DeploymentId(self.deployment_id);

        let version = Version::parse(&self.version).map_err(|error| {
            CoreError::InternalError(format!(
                "backup {} has an unreadable version: {error}",
                self.id
            ))
        })?;

        // Rebuilt through the prefix rather than read back as a path. Two ways
        // to address an archive is one too many, and this is the weaker one:
        // a stored path that disagreed with its own columns would point
        // somewhere nobody meant.
        let location = ArchivePrefix::new(organisation_id, deployment_id)
            .object(&self.object_key)
            .map_err(|error| {
                CoreError::InternalError(format!(
                    "backup {} holds a key that cannot be addressed: {error}",
                    self.id
                ))
            })?;

        let size_bytes = u64::try_from(self.size_bytes)
            .ok()
            .and_then(NonZeroU64::new)
            .ok_or_else(|| {
                CoreError::InternalError(format!(
                    "backup {} records a size of {} bytes, which is not an archive",
                    self.id, self.size_bytes
                ))
            })?;

        Ok(Backup {
            id: BackupId(self.id),
            deployment_id,
            organisation_id,
            release: ReleaseId::new(DeploymentKind::try_from(self.kind.as_str())?, version),
            postgres_major: PostgresMajor(self.postgres_major.unsigned_abs()),
            method: BackupMethod::try_from(self.method.as_str())?,
            key: KeyRef::new(
                ProviderName::new(self.key_provider),
                KeyName::new(self.key_name)?,
                KeyVersion::new(self.key_version.unsigned_abs()),
            ),
            location,
            size_bytes,
            started_at: self.started_at,
            finished_at: self.finished_at,
        })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[repository(domain = Backup, backend = Postgres)]
pub struct PostgresBackupRepository<'tx> {
    tx: SharedTx<'tx>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'tx> PostgresBackupRepository<'tx> {
    pub fn new(tx: &SharedTx<'tx>) -> Self {
        Self { tx: tx.clone() }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl BackupRepository for PostgresBackupRepository<'_> {
    async fn record(&self, backup: Backup) -> Result<(), CoreError> {
        let mut tx = self.tx.lock().await;

        sqlx::query!(
            r#"
            INSERT INTO backups (
                id,
                deployment_id,
                organisation_id,
                kind,
                version,
                postgres_major,
                method,
                key_provider,
                key_name,
                key_version,
                object_key,
                size_bytes,
                started_at,
                finished_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#,
            backup.id.0,
            backup.deployment_id.0,
            backup.organisation_id.0,
            backup.release.kind.to_string(),
            backup.release.version.to_string(),
            backup.postgres_major.0 as i32,
            backup.method.to_string(),
            backup.key.provider.as_str(),
            backup.key.name.as_str(),
            backup.key.version.value() as i32,
            backup.location.key().as_str(),
            backup.size_bytes.get() as i64,
            backup.started_at,
            backup.finished_at,
        )
        .execute(&mut ***tx)
        .await
        .map_err(|error| CoreError::DatabaseError {
            message: error.to_string(),
        })?;

        Ok(())
    }

    async fn get(&self, id: &BackupId) -> Result<Option<Backup>, CoreError> {
        let mut tx = self.tx.lock().await;

        let row = sqlx::query_as!(
            BackupRow,
            r#"
            SELECT id,
                   deployment_id,
                   organisation_id,
                   kind,
                   version,
                   postgres_major,
                   method,
                   key_provider,
                   key_name,
                   key_version,
                   object_key,
                   size_bytes,
                   started_at,
                   finished_at
            FROM backups
            WHERE id = $1
            "#,
            id.0
        )
        .fetch_optional(&mut ***tx)
        .await
        .map_err(|error| CoreError::DatabaseError {
            message: error.to_string(),
        })?;

        row.map(BackupRow::into_backup).transpose()
    }

    async fn list_for_deployment(
        &self,
        deployment: &DeploymentId,
    ) -> Result<Vec<Backup>, CoreError> {
        let mut tx = self.tx.lock().await;

        let rows = sqlx::query_as!(
            BackupRow,
            r#"
            SELECT id,
                   deployment_id,
                   organisation_id,
                   kind,
                   version,
                   postgres_major,
                   method,
                   key_provider,
                   key_name,
                   key_version,
                   object_key,
                   size_bytes,
                   started_at,
                   finished_at
            FROM backups
            WHERE deployment_id = $1
            ORDER BY finished_at DESC
            "#,
            deployment.0
        )
        .fetch_all(&mut ***tx)
        .await
        .map_err(|error| CoreError::DatabaseError {
            message: error.to_string(),
        })?;

        rows.into_iter().map(BackupRow::into_backup).collect()
    }

    async fn forget(&self, id: &BackupId) -> Result<(), CoreError> {
        let mut tx = self.tx.lock().await;

        sqlx::query!("DELETE FROM backups WHERE id = $1", id.0)
            .execute(&mut ***tx)
            .await
            .map_err(|error| CoreError::DatabaseError {
                message: error.to_string(),
            })?;

        Ok(())
    }
}
