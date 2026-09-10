use chrono::{DateTime, Utc};
use sqlx::FromRow;

use aether_domain::{
    CoreError,
    catalog::{
        BreakingRisk, Release, ReleaseId, ReleaseNotes, ReleaseStatus, ports::ReleaseRepository,
    },
    deployments::DeploymentKind,
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

        Ok(Release {
            id: ReleaseId::new(kind, version),
            status: parse_status(&self.status)?,
            risk: parse_risk(&self.risk)?,
            notes: ReleaseNotes(self.notes),
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
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

        // ON CONFLICT DO NOTHING rather than a SELECT first: two operators
        // publishing the same version at once would both see it missing, and
        // the second would overwrite what the first wrote.
        let inserted = {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            INSERT INTO releases (kind, version, status, risk, notes, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (kind, version) DO NOTHING
            "#,
                kind,
                version,
                status_to_row(release.status),
                risk_to_row(release.risk),
                release.notes.0,
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
            SELECT kind, version, status, risk, notes, created_at, updated_at
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
            SELECT kind, version, status, risk, notes, created_at, updated_at
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

        let updated = {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            UPDATE releases
            SET status = $3,
                risk = $4,
                notes = $5,
                updated_at = $6
            WHERE kind = $1 AND version = $2
            "#,
                kind,
                version,
                status_to_row(release.status),
                risk_to_row(release.risk),
                release.notes.0,
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
