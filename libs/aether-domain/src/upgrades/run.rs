use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    CoreError,
    deployments::DeploymentId,
    upgrades::ports::AcceptedUpgrade,
    user::UserId,
    version::{Version, VersionChange},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, ToSchema)]
pub struct UpgradeRunId(pub Uuid);

/// What asked for the upgrade.
///
/// A plain `Option<UserId>` field would let a manual run exist without
/// naming who asked, or a scheduled one carry a user by accident. Tagging the
/// two apart makes both states unrepresentable rather than merely
/// discouraged -- the whole reason this run is kept at all is to say who
/// asked, and a nullable field cannot promise that.
///
/// There is no scheduler yet. `Scheduled` is modelled anyway, because it is
/// the field the next chantier reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UpgradeTrigger {
    Manual { by: UserId },
    Scheduled,
}

/// How a concluded run ended.
///
/// The operator side is only now learning to tell a rollback from a plain
/// failure, so the two stay distinct here even though nothing downstream
/// reads the difference yet -- collapsing them into one `Failed` would lose
/// the distinction at the one point it is still available to record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeRunOutcome {
    Succeeded,
    Failed,
    RolledBack,
}

impl UpgradeRunOutcome {
    /// The name this outcome is known by outside this type, e.g. in an error
    /// message or a stored row. Not `{:?}` lowercased: that reads `RolledBack`
    /// as `rolledback`, which is not the same word as `rolled_back`.
    fn label(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::RolledBack => "rolled_back",
        }
    }
}

/// One attempt to move a deployment from one version to another.
///
/// Identified by its own id, not the deployment's. A deployment's `status`
/// and `version` hold exactly one attempt at a time, so the moment a second
/// upgrade starts, the first stops being visible there. Keying this record
/// the same way would reproduce the exact loss it exists to fix: a failed
/// attempt overwritten by the one that replaced it.
///
/// `change` travels with the run rather than being recomputed from
/// `from_version` and `to_version` on demand, for the same reason
/// [`AcceptedUpgrade`] carries it: it is a decision made once, by the service
/// that accepted the upgrade, and re-deriving it here is a second place for
/// that decision to drift from the first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct UpgradeRun {
    pub id: UpgradeRunId,
    pub deployment_id: DeploymentId,
    pub from_version: Version,
    pub to_version: Version,
    pub change: VersionChange,
    pub trigger: UpgradeTrigger,
    pub started_at: DateTime<Utc>,
    pub outcome: Option<UpgradeRunOutcome>,
    /// Why it ended that way. Carried for `Failed` and `RolledBack`; a
    /// success needs no explanation.
    pub detail: Option<String>,
    pub ended_at: Option<DateTime<Utc>>,
}

impl UpgradeRun {
    /// Starts a run for an upgrade a service has already accepted.
    ///
    /// The "from" is read off `accepted.deployment`, which is the deployment
    /// as it stood the instant the upgrade was accepted -- not a copy fetched
    /// again later, which could already have moved on.
    pub fn start(
        id: UpgradeRunId,
        accepted: &AcceptedUpgrade,
        to: Version,
        trigger: UpgradeTrigger,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            deployment_id: accepted.deployment.id,
            from_version: accepted.deployment.version.clone(),
            to_version: to,
            change: accepted.change,
            trigger,
            started_at: at,
            outcome: None,
            detail: None,
            ended_at: None,
        }
    }

    pub fn is_concluded(&self) -> bool {
        self.outcome.is_some()
    }

    pub fn succeed(&mut self, at: DateTime<Utc>) -> Result<(), CoreError> {
        self.conclude(UpgradeRunOutcome::Succeeded, None, at)
    }

    pub fn fail(&mut self, reason: impl Into<String>, at: DateTime<Utc>) -> Result<(), CoreError> {
        self.conclude(UpgradeRunOutcome::Failed, Some(reason.into()), at)
    }

    pub fn roll_back(
        &mut self,
        reason: impl Into<String>,
        at: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        self.conclude(UpgradeRunOutcome::RolledBack, Some(reason.into()), at)
    }

    /// Records how the run ended.
    ///
    /// A run concludes once. Unlike [`crate::catalog::Release`], whose
    /// statuses form a line a caller may walk forward along, `Succeeded`,
    /// `Failed` and `RolledBack` have no order between them -- there is no
    /// "further" outcome a concluded run could still move to. So the rule
    /// is not "no step backwards", it is "no second step at all".
    fn conclude(
        &mut self,
        outcome: UpgradeRunOutcome,
        detail: Option<String>,
        at: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        if let Some(existing) = self.outcome {
            return Err(CoreError::UpgradeRunAlreadyConcluded {
                run: self.id.0,
                outcome: existing.label().to_string(),
            });
        }

        self.outcome = Some(outcome);
        self.detail = detail;
        self.ended_at = Some(at);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        dataplane::value_objects::{DataPlaneId, DeploymentResources},
        deployments::{Deployment, DeploymentKind, DeploymentName, DeploymentStatus},
        organisation::OrganisationId,
    };

    fn at() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .expect("a valid instant")
            .with_timezone(&Utc)
    }

    fn later() -> DateTime<Utc> {
        at() + chrono::Duration::minutes(10)
    }

    fn accepted(from: Version, change: VersionChange) -> AcceptedUpgrade {
        AcceptedUpgrade {
            deployment: Deployment {
                id: DeploymentId(Uuid::from_u128(1)),
                organisation_id: OrganisationId(Uuid::from_u128(2)),
                dataplane_id: DataPlaneId(Uuid::from_u128(3)),
                name: DeploymentName("auth".to_string()),
                kind: DeploymentKind::Ferriskey,
                version: from,
                status: DeploymentStatus::Upgrading,
                namespace: "ns".to_string(),
                resources: DeploymentResources::DEFAULT,
                created_by: UserId(Uuid::from_u128(4)),
                created_at: at(),
                updated_at: at(),
                deployed_at: None,
                deleted_at: None,
                auto_upgrade: Default::default(),
                maintenance_window: None,
            },
            change,
        }
    }

    fn run() -> UpgradeRun {
        UpgradeRun::start(
            UpgradeRunId(Uuid::from_u128(99)),
            &accepted(Version::new(26, 0, 0), VersionChange::Patch),
            Version::new(26, 0, 1),
            UpgradeTrigger::Manual {
                by: UserId(Uuid::from_u128(5)),
            },
            at(),
        )
    }

    /// The two facts a support conversation needs and a deployment row
    /// cannot answer: which versions, and what kind of step.
    #[test]
    fn a_run_starts_recording_both_versions_and_the_step_between_them() {
        let run = run();

        assert_eq!(run.from_version, Version::new(26, 0, 0));
        assert_eq!(run.to_version, Version::new(26, 0, 1));
        assert_eq!(run.change, VersionChange::Patch);
        assert_eq!(run.started_at, at());
    }

    #[test]
    fn a_run_starts_unresolved() {
        let run = run();

        assert!(!run.is_concluded());
        assert_eq!(run.outcome, None);
        assert_eq!(run.ended_at, None);
    }

    /// The version a run records as "from" is the one the deployment ran the
    /// moment the upgrade was accepted, not whatever it runs by the time
    /// anyone reads this run back.
    #[test]
    fn the_from_version_is_read_off_the_accepted_deployment() {
        let run = UpgradeRun::start(
            UpgradeRunId(Uuid::from_u128(1)),
            &accepted(Version::new(25, 4, 2), VersionChange::Minor),
            Version::new(26, 0, 0),
            UpgradeTrigger::Scheduled,
            at(),
        );

        assert_eq!(run.from_version, Version::new(25, 4, 2));
        assert_eq!(run.change, VersionChange::Minor);
    }

    #[test]
    fn succeeding_concludes_the_run_without_a_reason() {
        let mut run = run();

        run.succeed(later()).expect("an in-progress run may settle");

        assert_eq!(run.outcome, Some(UpgradeRunOutcome::Succeeded));
        assert_eq!(run.detail, None);
        assert_eq!(run.ended_at, Some(later()));
    }

    /// Why it failed is exactly what a deployment's own status cannot say
    /// once the next upgrade has overwritten it.
    #[test]
    fn failing_records_the_reason() {
        let mut run = run();

        run.fail("the operator reported a crash loop", later())
            .expect("an in-progress run may fail");

        assert_eq!(run.outcome, Some(UpgradeRunOutcome::Failed));
        assert_eq!(
            run.detail,
            Some("the operator reported a crash loop".to_string())
        );
    }

    /// A rollback and a plain failure both end the run, but they are not the
    /// same fact and must not collapse into one.
    #[test]
    fn rolling_back_is_distinct_from_failing() {
        let mut failed = run();
        failed.fail("crash loop", later()).expect("fails");

        let mut rolled_back = run();
        rolled_back
            .roll_back("reverted after a bad canary", later())
            .expect("rolls back");

        assert_ne!(failed.outcome, rolled_back.outcome);
        assert_eq!(rolled_back.outcome, Some(UpgradeRunOutcome::RolledBack));
    }

    /// The rule the whole `Option<UpgradeRunOutcome>` exists for: once this
    /// run says how it ended, that is not renegotiable by whatever happens
    /// next.
    #[test]
    fn a_concluded_run_refuses_to_conclude_again() {
        let mut run = run();
        run.succeed(later()).expect("first conclusion");

        let error = run
            .fail("retried", later())
            .expect_err("a settled run does not re-open");

        assert!(matches!(
            error,
            CoreError::UpgradeRunAlreadyConcluded { .. }
        ));
        assert_eq!(
            run.outcome,
            Some(UpgradeRunOutcome::Succeeded),
            "the original outcome was not disturbed"
        );
    }

    /// Whichever outcome got there first must survive being asked to
    /// conclude again as anything else, not only as the same thing.
    #[test]
    fn every_outcome_refuses_every_further_conclusion() {
        let mut succeeded = run();
        succeeded
            .succeed(later())
            .expect("the first conclusion succeeds");
        assert!(matches!(
            succeeded.fail("retried", later()),
            Err(CoreError::UpgradeRunAlreadyConcluded { .. })
        ));

        let mut failed = run();
        failed
            .fail("first failure", later())
            .expect("the first conclusion succeeds");
        assert!(matches!(
            failed.roll_back("reverted", later()),
            Err(CoreError::UpgradeRunAlreadyConcluded { .. })
        ));

        let mut rolled_back = run();
        rolled_back
            .roll_back("reverted", later())
            .expect("the first conclusion succeeds");
        assert!(matches!(
            rolled_back.succeed(later()),
            Err(CoreError::UpgradeRunAlreadyConcluded { .. })
        ));
    }

    /// The refusal names the run and what it already concluded as, because
    /// whoever reads it is looking at a history, not a single row.
    #[test]
    fn the_refusal_names_the_run_and_its_outcome() {
        let mut run = run();
        run.roll_back("reverted after a bad canary", later())
            .expect("rolls back");

        let error = run
            .succeed(later())
            .expect_err("a rolled back run does not also succeed");
        let message = error.to_string();

        assert!(message.contains(&run.id.0.to_string()), "{message}");
        assert!(message.contains("rolled_back"), "{message}");
    }

    #[test]
    fn triggers_serialise_distinctly_by_kind() {
        let manual = UpgradeTrigger::Manual {
            by: UserId(Uuid::from_u128(1)),
        };

        assert!(
            serde_json::to_string(&manual)
                .expect("serialises")
                .contains("\"manual\"")
        );
        assert!(
            serde_json::to_string(&UpgradeTrigger::Scheduled)
                .expect("serialises")
                .contains("\"scheduled\"")
        );
    }

    #[test]
    fn outcomes_serialise_in_the_same_case_as_the_rest_of_the_api() {
        assert_eq!(
            serde_json::to_string(&UpgradeRunOutcome::RolledBack).expect("serialises"),
            "\"rolled_back\""
        );
    }
}
