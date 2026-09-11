use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use aether_domain::{
    CoreError,
    deployments::DeploymentId,
    upgrades::{
        run::{UpgradeRun, UpgradeRunId, UpgradeRunOutcome, UpgradeTrigger},
        run_ports::UpgradeRunRepository,
    },
    user::UserId,
    version::{Version, VersionChange},
};
use aether_macros::repository;
use aether_persistence::SharedTx;

#[derive(FromRow)]
struct UpgradeRunRow {
    id: Uuid,
    deployment_id: Uuid,
    from_version: String,
    to_version: String,
    change: String,
    steps: Vec<String>,
    trigger_kind: String,
    triggered_by: Option<Uuid>,
    started_at: DateTime<Utc>,
    outcome: Option<String>,
    detail: Option<String>,
    ended_at: Option<DateTime<Utc>>,
}

impl UpgradeRunRow {
    fn into_run(self) -> Result<UpgradeRun, CoreError> {
        let from_version = parse_version(&self.from_version)?;
        let to_version = parse_version(&self.to_version)?;

        let steps = parse_steps(&self.steps, &to_version)?;

        Ok(UpgradeRun {
            id: UpgradeRunId(self.id),
            deployment_id: DeploymentId(self.deployment_id),
            from_version,
            to_version,
            steps,
            change: parse_change(&self.change)?,
            trigger: parse_trigger(&self.trigger_kind, self.triggered_by)?,
            started_at: self.started_at,
            outcome: self.outcome.as_deref().map(parse_outcome).transpose()?,
            detail: self.detail,
            ended_at: self.ended_at,
        })
    }
}

fn parse_version(value: &str) -> Result<Version, CoreError> {
    Version::parse(value).map_err(|e| {
        CoreError::InternalError(format!(
            "upgrade run has an unreadable version '{value}': {e}"
        ))
    })
}

/// A run written before the path was recorded has no steps, and its target is
/// the whole route by construction. Reading that back as an empty path would
/// tell a screen an upgrade is on step 1 of 0.
fn parse_steps(stored: &[String], to_version: &Version) -> Result<Vec<Version>, CoreError> {
    if stored.is_empty() {
        return Ok(vec![to_version.clone()]);
    }

    stored.iter().map(|step| parse_version(step)).collect()
}

fn change_to_row(change: VersionChange) -> &'static str {
    match change {
        VersionChange::Patch => "patch",
        VersionChange::Minor => "minor",
        VersionChange::Major => "major",
    }
}

fn parse_change(value: &str) -> Result<VersionChange, CoreError> {
    match value {
        "patch" => Ok(VersionChange::Patch),
        "minor" => Ok(VersionChange::Minor),
        "major" => Ok(VersionChange::Major),
        other => Err(CoreError::InternalError(format!(
            "unknown upgrade change '{other}'"
        ))),
    }
}

/// Splits a trigger into the two nullable columns it is stored as.
///
/// Not validated against each other in the schema (see the migration), so
/// this is the one place a manual trigger without a `triggered_by` could slip
/// through -- and it cannot, because [`UpgradeTrigger::Manual`] has nowhere
/// else to get its `by` from.
fn trigger_to_row(trigger: &UpgradeTrigger) -> (&'static str, Option<Uuid>) {
    match trigger {
        UpgradeTrigger::Manual { by } => ("manual", Some(by.0)),
        UpgradeTrigger::Scheduled => ("scheduled", None),
    }
}

fn parse_trigger(kind: &str, triggered_by: Option<Uuid>) -> Result<UpgradeTrigger, CoreError> {
    match (kind, triggered_by) {
        ("manual", Some(by)) => Ok(UpgradeTrigger::Manual { by: UserId(by) }),
        ("scheduled", None) => Ok(UpgradeTrigger::Scheduled),
        ("manual", None) => Err(CoreError::InternalError(
            "upgrade run is manual but names nobody".to_string(),
        )),
        (other, _) => Err(CoreError::InternalError(format!(
            "unknown upgrade trigger '{other}'"
        ))),
    }
}

fn outcome_to_row(outcome: UpgradeRunOutcome) -> &'static str {
    match outcome {
        UpgradeRunOutcome::Succeeded => "succeeded",
        UpgradeRunOutcome::Failed => "failed",
        UpgradeRunOutcome::RolledBack => "rolled_back",
    }
}

fn parse_outcome(value: &str) -> Result<UpgradeRunOutcome, CoreError> {
    match value {
        "succeeded" => Ok(UpgradeRunOutcome::Succeeded),
        "failed" => Ok(UpgradeRunOutcome::Failed),
        "rolled_back" => Ok(UpgradeRunOutcome::RolledBack),
        other => Err(CoreError::InternalError(format!(
            "unknown upgrade outcome '{other}'"
        ))),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[repository(domain = UpgradeRun, backend = Postgres)]
pub struct PostgresUpgradeRunRepository<'tx> {
    tx: SharedTx<'tx>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'tx> PostgresUpgradeRunRepository<'tx> {
    pub fn new(tx: &SharedTx<'tx>) -> Self {
        Self { tx: tx.clone() }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl UpgradeRunRepository for PostgresUpgradeRunRepository<'_> {
    async fn insert(&self, run: UpgradeRun) -> Result<(), CoreError> {
        let (trigger_kind, triggered_by) = trigger_to_row(&run.trigger);
        let outcome = run.outcome.map(outcome_to_row);

        let mut tx = self.tx.lock().await;
        sqlx::query!(
            r#"
            INSERT INTO upgrade_runs (
                id, deployment_id, from_version, to_version, change, steps,
                trigger_kind, triggered_by, started_at, outcome, detail, ended_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
            run.id.0,
            run.deployment_id.0,
            run.from_version.to_string(),
            run.to_version.to_string(),
            change_to_row(run.change),
            &run.steps
                .iter()
                .map(|step| step.to_string())
                .collect::<Vec<_>>(),
            trigger_kind,
            triggered_by,
            run.started_at,
            outcome,
            run.detail,
            run.ended_at,
        )
        .execute(&mut ***tx)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to insert upgrade run: {e}"),
        })?;

        Ok(())
    }

    async fn get(&self, id: UpgradeRunId) -> Result<Option<UpgradeRun>, CoreError> {
        let row = {
            let mut tx = self.tx.lock().await;
            sqlx::query_as!(
                UpgradeRunRow,
                r#"
            SELECT id, deployment_id, from_version, to_version, change,
                   steps as "steps!: Vec<String>",
                   trigger_kind, triggered_by, started_at, outcome, detail, ended_at
            FROM upgrade_runs
            WHERE id = $1
            "#,
                id.0
            )
            .fetch_optional(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to get upgrade run: {e}"),
        })?;

        row.map(|row| row.into_run()).transpose()
    }

    async fn update(&self, run: &UpgradeRun) -> Result<(), CoreError> {
        let outcome = run.outcome.map(outcome_to_row);

        let updated = {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            UPDATE upgrade_runs
            SET outcome = $2,
                detail = $3,
                ended_at = $4
            WHERE id = $1
            "#,
                run.id.0,
                outcome,
                run.detail,
                run.ended_at,
            )
            .execute(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to update upgrade run: {e}"),
        })?
        .rows_affected();

        if updated == 0 {
            return Err(CoreError::UpgradeRunNotFound { id: run.id.0 });
        }

        Ok(())
    }

    async fn list_for_deployment(
        &self,
        deployment_id: DeploymentId,
    ) -> Result<Vec<UpgradeRun>, CoreError> {
        let rows = {
            let mut tx = self.tx.lock().await;
            sqlx::query_as!(
                UpgradeRunRow,
                r#"
            SELECT id, deployment_id, from_version, to_version, change,
                   steps as "steps!: Vec<String>",
                   trigger_kind, triggered_by, started_at, outcome, detail, ended_at
            FROM upgrade_runs
            WHERE deployment_id = $1
            ORDER BY started_at DESC
            "#,
                deployment_id.0
            )
            .fetch_all(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to list upgrade runs: {e}"),
        })?;

        rows.into_iter()
            .map(UpgradeRunRow::into_run)
            .collect::<Result<Vec<UpgradeRun>, CoreError>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_round_trips_through_its_row_value() {
        for change in [
            VersionChange::Patch,
            VersionChange::Minor,
            VersionChange::Major,
        ] {
            assert_eq!(parse_change(change_to_row(change)).expect("known"), change);
        }
    }

    #[test]
    fn outcome_round_trips_through_its_row_value() {
        for outcome in [
            UpgradeRunOutcome::Succeeded,
            UpgradeRunOutcome::Failed,
            UpgradeRunOutcome::RolledBack,
        ] {
            assert_eq!(
                parse_outcome(outcome_to_row(outcome)).expect("known"),
                outcome
            );
        }
    }

    #[test]
    fn a_manual_trigger_round_trips_with_who_asked() {
        let trigger = UpgradeTrigger::Manual {
            by: UserId(Uuid::from_u128(7)),
        };
        let (kind, by) = trigger_to_row(&trigger);

        assert_eq!(parse_trigger(kind, by).expect("known"), trigger);
    }

    #[test]
    fn a_scheduled_trigger_round_trips_with_nobody() {
        let (kind, by) = trigger_to_row(&UpgradeTrigger::Scheduled);

        assert_eq!(
            parse_trigger(kind, by).expect("known"),
            UpgradeTrigger::Scheduled
        );
    }

    /// A row the schema should not be able to produce, but the check
    /// constraint on `trigger_kind` alone cannot rule out a NULL
    /// `triggered_by` beside it. Saying so beats guessing a user.
    #[test]
    fn a_manual_row_naming_nobody_is_reported_rather_than_guessed() {
        assert!(parse_trigger("manual", None).is_err());
    }

    #[test]
    fn an_unknown_row_value_is_reported_rather_than_guessed() {
        assert!(parse_change("hotfix").is_err());
        assert!(parse_outcome("aborted").is_err());
        assert!(parse_trigger("cron", None).is_err());
    }
}
