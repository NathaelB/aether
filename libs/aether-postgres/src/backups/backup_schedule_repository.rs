use chrono::{DateTime, Utc};
use sqlx::FromRow;
use std::{num::NonZeroU32, str::FromStr};
use uuid::Uuid;

use aether_domain::{
    CoreError,
    backups::{BackupMethod, BackupSchedule, Cadence, Retention, ports::BackupScheduleRepository},
    deployments::DeploymentId,
    organisation::OrganisationId,
};
use aether_macros::repository;
use aether_persistence::SharedTx;
use chrono_tz::Tz;

#[derive(FromRow)]
struct ScheduleRow {
    deployment_id: Uuid,
    organisation_id: Uuid,
    cadence: serde_json::Value,
    zone: String,
    keep_last: i32,
    keep_for_days: i64,
    method: String,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl ScheduleRow {
    fn into_schedule(self) -> Result<BackupSchedule, CoreError> {
        let cadence: Cadence = serde_json::from_value(self.cadence).map_err(|error| {
            CoreError::InternalError(format!(
                "backup schedule for deployment {} has an unreadable cadence: {error}",
                self.deployment_id
            ))
        })?;

        // A zone that no longer exists in the database is not a reason to hide
        // the schedule: it is a reason to say which deployment has it, so
        // somebody can fix it.
        let zone = Tz::from_str(&self.zone).map_err(|error| {
            CoreError::InternalError(format!(
                "backup schedule for deployment {} names the zone '{}', which is not a zone: {error}",
                self.deployment_id, self.zone
            ))
        })?;

        let keep_last = NonZeroU32::new(self.keep_last.unsigned_abs()).ok_or_else(|| {
            CoreError::InternalError(format!(
                "backup schedule for deployment {} keeps zero archives, which no policy may ask for",
                self.deployment_id
            ))
        })?;

        Ok(BackupSchedule {
            deployment_id: DeploymentId(self.deployment_id),
            organisation_id: OrganisationId(self.organisation_id),
            cadence,
            zone,
            retention: Retention::new(keep_last, self.keep_for_days)?,
            method: BackupMethod::try_from(self.method.as_str())?,
            enabled: self.enabled,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[repository(domain = BackupSchedule, backend = Postgres)]
pub struct PostgresBackupScheduleRepository<'tx> {
    tx: SharedTx<'tx>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'tx> PostgresBackupScheduleRepository<'tx> {
    pub fn new(tx: &SharedTx<'tx>) -> Self {
        Self { tx: tx.clone() }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl BackupScheduleRepository for PostgresBackupScheduleRepository<'_> {
    async fn save(&self, schedule: BackupSchedule) -> Result<(), CoreError> {
        let cadence = serde_json::to_value(&schedule.cadence).map_err(|error| {
            CoreError::InternalError(format!("a cadence that cannot be written down: {error}"))
        })?;

        let mut tx = self.tx.lock().await;

        sqlx::query!(
            r#"
            INSERT INTO backup_schedules (
                deployment_id,
                organisation_id,
                cadence,
                zone,
                keep_last,
                keep_for_days,
                method,
                enabled,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (deployment_id) DO UPDATE SET
                cadence = EXCLUDED.cadence,
                zone = EXCLUDED.zone,
                keep_last = EXCLUDED.keep_last,
                keep_for_days = EXCLUDED.keep_for_days,
                method = EXCLUDED.method,
                enabled = EXCLUDED.enabled,
                updated_at = EXCLUDED.updated_at
            "#,
            schedule.deployment_id.0,
            schedule.organisation_id.0,
            cadence,
            schedule.zone.name(),
            schedule.retention.keep_last().get() as i32,
            schedule.retention.keep_for_days(),
            schedule.method.to_string(),
            schedule.enabled,
            schedule.created_at,
            schedule.updated_at,
        )
        .execute(&mut ***tx)
        .await
        .map_err(|error| CoreError::DatabaseError {
            message: error.to_string(),
        })?;

        Ok(())
    }

    async fn get(&self, deployment: &DeploymentId) -> Result<Option<BackupSchedule>, CoreError> {
        let mut tx = self.tx.lock().await;

        let row = sqlx::query_as!(
            ScheduleRow,
            r#"
            SELECT deployment_id,
                   organisation_id,
                   cadence,
                   zone,
                   keep_last,
                   keep_for_days,
                   method,
                   enabled,
                   created_at,
                   updated_at
            FROM backup_schedules
            WHERE deployment_id = $1
            "#,
            deployment.0
        )
        .fetch_optional(&mut ***tx)
        .await
        .map_err(|error| CoreError::DatabaseError {
            message: error.to_string(),
        })?;

        row.map(ScheduleRow::into_schedule).transpose()
    }

    async fn list_enabled(&self) -> Result<Vec<BackupSchedule>, CoreError> {
        let mut tx = self.tx.lock().await;

        let rows = sqlx::query_as!(
            ScheduleRow,
            r#"
            SELECT deployment_id,
                   organisation_id,
                   cadence,
                   zone,
                   keep_last,
                   keep_for_days,
                   method,
                   enabled,
                   created_at,
                   updated_at
            FROM backup_schedules
            WHERE enabled
            "#
        )
        .fetch_all(&mut ***tx)
        .await
        .map_err(|error| CoreError::DatabaseError {
            message: error.to_string(),
        })?;

        rows.into_iter().map(ScheduleRow::into_schedule).collect()
    }
}
