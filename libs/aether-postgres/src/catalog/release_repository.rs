use std::str::FromStr;

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use aether_domain::{
    CoreError,
    catalog::{
        BreakingRisk, Release, ReleaseId, ReleaseNotes, ReleaseStatus, Rollout, RolloutPercentage,
        ports::ReleaseRepository,
    },
    deployments::DeploymentKind,
    organisation::{OrganisationId, value_objects::Plan},
    version::Version,
};
use aether_macros::repository;
use aether_persistence::SharedTx;

#[derive(FromRow)]
struct ReleaseRow {
    kind: String,
    version: String,
    status: String,
    risk: String,
    notes: String,
    rollout_percentage: i16,
    rollout_plans: Option<Vec<String>>,
    rollout_pilot_organisations: Vec<Uuid>,
    minimum_operator_version: Option<String>,
    steps_through: Vec<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl ReleaseRow {
    fn into_release(self) -> Result<Release, CoreError> {
        let kind = DeploymentKind::try_from(self.kind.as_str())?;
        let version = Version::parse(&self.version).map_err(|e| {
            CoreError::InternalError(format!(
                "release {} {} has an unreadable version: {e}",
                self.kind, self.version
            ))
        })?;
        let rollout = parse_rollout(
            &kind,
            &version,
            self.rollout_percentage,
            self.rollout_plans,
            self.rollout_pilot_organisations,
        )?;
        let minimum_operator_version = self
            .minimum_operator_version
            .map(|raw| parse_release_version(&kind, &version, "minimum operator version", &raw))
            .transpose()?;
        let steps_through = self
            .steps_through
            .iter()
            .map(|raw| parse_release_version(&kind, &version, "step", raw))
            .collect::<Result<Vec<Version>, CoreError>>()?;

        Ok(Release {
            id: ReleaseId::new(kind, version),
            status: parse_status(&self.status)?,
            risk: parse_risk(&self.risk)?,
            notes: ReleaseNotes(self.notes),
            rollout,
            minimum_operator_version,
            steps_through,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

fn parse_release_version(
    kind: &DeploymentKind,
    version: &Version,
    field: &str,
    raw: &str,
) -> Result<Version, CoreError> {
    Version::parse(raw).map_err(|e| {
        CoreError::InternalError(format!(
            "release {kind} {version} has an unreadable {field} '{raw}': {e}"
        ))
    })
}

fn parse_rollout(
    kind: &DeploymentKind,
    version: &Version,
    percentage: i16,
    plans: Option<Vec<String>>,
    pilot_organisations: Vec<Uuid>,
) -> Result<Rollout, CoreError> {
    let percentage = u8::try_from(percentage)
        .ok()
        .and_then(|value| RolloutPercentage::new(value).ok())
        .ok_or_else(|| {
            CoreError::InternalError(format!(
                "release {kind} {version} has an invalid rollout percentage: {percentage}"
            ))
        })?;

    let plans = plans
        .map(|raw| {
            raw.iter()
                .map(|value| Plan::from_str(value))
                .collect::<Result<Vec<Plan>, CoreError>>()
        })
        .transpose()?;

    let pilot_organisations = pilot_organisations
        .into_iter()
        .map(OrganisationId)
        .collect();

    Ok(Rollout::new(percentage, plans, pilot_organisations))
}

fn status_to_row(status: ReleaseStatus) -> &'static str {
    match status {
        ReleaseStatus::Upcoming => "upcoming",
        ReleaseStatus::Available => "available",
        ReleaseStatus::Deprecated => "deprecated",
        ReleaseStatus::Withdrawn => "withdrawn",
    }
}

fn parse_status(value: &str) -> Result<ReleaseStatus, CoreError> {
    match value {
        "upcoming" => Ok(ReleaseStatus::Upcoming),
        "available" => Ok(ReleaseStatus::Available),
        "deprecated" => Ok(ReleaseStatus::Deprecated),
        "withdrawn" => Ok(ReleaseStatus::Withdrawn),
        other => Err(CoreError::InternalError(format!(
            "unknown release status '{other}'"
        ))),
    }
}

fn risk_to_row(risk: BreakingRisk) -> &'static str {
    match risk {
        BreakingRisk::None => "none",
        BreakingRisk::Config => "config",
        BreakingRisk::Breaking => "breaking",
    }
}

fn parse_risk(value: &str) -> Result<BreakingRisk, CoreError> {
    match value {
        "none" => Ok(BreakingRisk::None),
        "config" => Ok(BreakingRisk::Config),
        "breaking" => Ok(BreakingRisk::Breaking),
        other => Err(CoreError::InternalError(format!(
            "unknown breaking risk '{other}'"
        ))),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[repository(domain = Release, backend = Postgres)]
pub struct PostgresReleaseRepository<'tx> {
    tx: SharedTx<'tx>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'tx> PostgresReleaseRepository<'tx> {
    pub fn new(tx: &SharedTx<'tx>) -> Self {
        Self { tx: tx.clone() }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ReleaseRepository for PostgresReleaseRepository<'_> {
    async fn insert(&self, release: Release) -> Result<(), CoreError> {
        let kind = release.id.kind.to_string();
        let version = release.id.version.to_string();
        let rollout_percentage = i16::from(release.rollout.percentage().value());
        let rollout_plans = plans_to_row(release.rollout.plans());
        let rollout_pilot_organisations = pilots_to_row(release.rollout.pilot_organisations());
        let minimum_operator_version = release
            .minimum_operator_version
            .as_ref()
            .map(Version::to_string);
        let steps_through = versions_to_row(&release.steps_through);

        // ON CONFLICT DO NOTHING rather than a SELECT first: two operators
        // publishing the same version at once would both see it missing, and
        // the second would overwrite what the first wrote.
        let inserted = {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            INSERT INTO releases (
                kind, version, status, risk, notes,
                rollout_percentage, rollout_plans, rollout_pilot_organisations,
                minimum_operator_version, steps_through,
                created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (kind, version) DO NOTHING
            "#,
                kind,
                version,
                status_to_row(release.status),
                risk_to_row(release.risk),
                release.notes.0,
                rollout_percentage,
                rollout_plans.as_deref(),
                &rollout_pilot_organisations,
                minimum_operator_version,
                &steps_through,
                release.created_at,
                release.updated_at,
            )
            .execute(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to insert release: {e}"),
        })?
        .rows_affected();

        if inserted == 0 {
            return Err(CoreError::ReleaseAlreadyExists {
                release: release.id.to_string(),
            });
        }

        Ok(())
    }

    async fn get(
        &self,
        kind: &DeploymentKind,
        version: &Version,
    ) -> Result<Option<Release>, CoreError> {
        let kind = kind.to_string();
        let version = version.to_string();

        let row = {
            let mut tx = self.tx.lock().await;
            sqlx::query_as!(
                ReleaseRow,
                r#"
            SELECT kind, version, status, risk, notes,
                   rollout_percentage, rollout_plans, rollout_pilot_organisations,
                   minimum_operator_version, steps_through,
                   created_at, updated_at
            FROM releases
            WHERE kind = $1 AND version = $2
            "#,
                kind,
                version
            )
            .fetch_optional(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to get release: {e}"),
        })?;

        row.map(|row| row.into_release()).transpose()
    }

    async fn list_for_kind(&self, kind: &DeploymentKind) -> Result<Vec<Release>, CoreError> {
        let kind = kind.to_string();

        // Ordered in Rust rather than in SQL: the version column is text, so
        // Postgres would sort 26.0.10 before 26.0.9, which is the ordering bug
        // the Version type exists to prevent.
        let rows = {
            let mut tx = self.tx.lock().await;
            sqlx::query_as!(
                ReleaseRow,
                r#"
            SELECT kind, version, status, risk, notes,
                   rollout_percentage, rollout_plans, rollout_pilot_organisations,
                   minimum_operator_version, steps_through,
                   created_at, updated_at
            FROM releases
            WHERE kind = $1
            "#,
                kind
            )
            .fetch_all(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to list releases: {e}"),
        })?;

        let mut releases = rows
            .into_iter()
            .map(|row| row.into_release())
            .collect::<Result<Vec<Release>, CoreError>>()?;

        releases.sort_by(|a, b| b.id.version.cmp(&a.id.version));

        Ok(releases)
    }

    async fn update(&self, release: &Release) -> Result<(), CoreError> {
        let kind = release.id.kind.to_string();
        let version = release.id.version.to_string();
        let rollout_percentage = i16::from(release.rollout.percentage().value());
        let rollout_plans = plans_to_row(release.rollout.plans());
        let rollout_pilot_organisations = pilots_to_row(release.rollout.pilot_organisations());
        let minimum_operator_version = release
            .minimum_operator_version
            .as_ref()
            .map(Version::to_string);
        let steps_through = versions_to_row(&release.steps_through);

        let updated = {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            UPDATE releases
            SET status = $3,
                risk = $4,
                notes = $5,
                rollout_percentage = $6,
                rollout_plans = $7,
                rollout_pilot_organisations = $8,
                minimum_operator_version = $9,
                steps_through = $10,
                updated_at = $11
            WHERE kind = $1 AND version = $2
            "#,
                kind,
                version,
                status_to_row(release.status),
                risk_to_row(release.risk),
                release.notes.0,
                rollout_percentage,
                rollout_plans.as_deref(),
                &rollout_pilot_organisations,
                minimum_operator_version,
                &steps_through,
                release.updated_at,
            )
            .execute(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to update release: {e}"),
        })?
        .rows_affected();

        if updated == 0 {
            return Err(CoreError::ReleaseNotFound {
                release: release.id.to_string(),
            });
        }

        Ok(())
    }
}

fn plans_to_row(plans: Option<&[Plan]>) -> Option<Vec<String>> {
    plans.map(|plans| plans.iter().map(Plan::to_string).collect())
}

fn pilots_to_row(pilot_organisations: &[OrganisationId]) -> Vec<Uuid> {
    pilot_organisations.iter().map(|id| id.0).collect()
}

fn versions_to_row(versions: &[Version]) -> Vec<String> {
    versions.iter().map(Version::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_round_trips_through_its_row_value() {
        for status in [
            ReleaseStatus::Upcoming,
            ReleaseStatus::Available,
            ReleaseStatus::Deprecated,
            ReleaseStatus::Withdrawn,
        ] {
            assert_eq!(parse_status(status_to_row(status)).expect("known"), status);
        }
    }

    #[test]
    fn risk_round_trips_through_its_row_value() {
        for risk in [
            BreakingRisk::None,
            BreakingRisk::Config,
            BreakingRisk::Breaking,
        ] {
            assert_eq!(parse_risk(risk_to_row(risk)).expect("known"), risk);
        }
    }

    /// A value the database should not be able to hold, but the check
    /// constraint is the only thing stopping it. Saying so beats mapping it to
    /// a plausible default and carrying the wrong status onward.
    #[test]
    fn an_unknown_row_value_is_reported_rather_than_guessed() {
        assert!(parse_status("retired").is_err());
        assert!(parse_risk("mild").is_err());
    }
}
