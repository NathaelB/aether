use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::dataplane::value_objects::DeploymentResources;
use crate::upgrades::policy::{AutoUpgradePolicy, MaintenanceWindow};
use crate::version::Version;
use crate::{
    CoreError, dataplane::value_objects::DataPlaneId, organisation::OrganisationId, user::UserId,
};

pub mod commands;
pub mod ports;
pub mod service;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct DeploymentId(pub Uuid);

impl FromStr for DeploymentId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::from_str(s).map(DeploymentId)
    }
}

impl From<Uuid> for DeploymentId {
    fn from(value: Uuid) -> Self {
        DeploymentId(value)
    }
}

impl fmt::Display for DeploymentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, ToSchema)]
pub struct DeploymentName(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentKind {
    Ferriskey,
    Keycloak,
}

impl fmt::Display for DeploymentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ferriskey => write!(f, "ferriskey"),
            Self::Keycloak => write!(f, "keycloak"),
        }
    }
}

impl TryFrom<&str> for DeploymentKind {
    type Error = CoreError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "ferriskey" => Ok(Self::Ferriskey),
            "keycloak" => Ok(Self::Keycloak),
            _ => Err(CoreError::InternalError(format!(
                "Invalid deployment kind: {}",
                value
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentStatus {
    Pending,
    Scheduling,
    InProgress,
    Successful,
    Failed,
    Maintenance,
    UpgradeRequired,
    Upgrading,
    /// Tear-down has been asked for and has not been confirmed.
    Deleting,
    /// The data plane reported that the resources are gone.
    ///
    /// Terminal. Distinct from `Deleting`, which is a deployment whose
    /// tear-down is in flight -- without the distinction a deletion that
    /// completed and one that never got anywhere look identical, and
    /// `Deleting` was where every deleted deployment stopped for ever.
    Deleted,
}

impl fmt::Display for DeploymentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Scheduling => write!(f, "scheduling"),
            Self::InProgress => write!(f, "in_progress"),
            Self::Successful => write!(f, "successful"),
            Self::Failed => write!(f, "failed"),
            Self::Maintenance => write!(f, "maintenance"),
            Self::UpgradeRequired => write!(f, "upgrade_required"),
            Self::Upgrading => write!(f, "upgrading"),
            Self::Deleting => write!(f, "deleting"),
            Self::Deleted => write!(f, "deleted"),
        }
    }
}

impl TryFrom<&str> for DeploymentStatus {
    type Error = CoreError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "scheduling" => Ok(Self::Scheduling),
            "in_progress" => Ok(Self::InProgress),
            "successful" => Ok(Self::Successful),
            "failed" => Ok(Self::Failed),
            "maintenance" => Ok(Self::Maintenance),
            "upgrade_required" => Ok(Self::UpgradeRequired),
            "upgrading" => Ok(Self::Upgrading),
            "deleting" => Ok(Self::Deleting),
            "deleted" => Ok(Self::Deleted),
            _ => Err(CoreError::InternalError(format!(
                "Invalid deployment status: {}",
                value
            ))),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Deployment {
    pub id: DeploymentId,
    pub organisation_id: OrganisationId,
    pub dataplane_id: DataPlaneId,
    pub name: DeploymentName,

    pub kind: DeploymentKind,
    pub version: Version,

    pub status: DeploymentStatus,
    pub namespace: String,

    /// What this deployment costs its data plane, and what its database is
    /// sized to. One value, so the room reserved at placement and the spec
    /// written into the IdentityInstance cannot disagree.
    pub resources: DeploymentResources,

    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub deployed_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,

    /// What the customer lets the platform apply without asking.
    pub auto_upgrade: AutoUpgradePolicy,

    /// When it may do so. Absent means never, whatever the policy says:
    /// declining to name a window is a choice, not an omission to fill in
    /// with a default nobody picked.
    pub maintenance_window: Option<MaintenanceWindow>,
}

impl Deployment {
    /// Records that the work has been handed to a data plane.
    ///
    /// Returns whether anything changed, so a caller can skip a write.
    ///
    /// This is the one transition the control plane can make on its own
    /// evidence. Herald acknowledging an action as `published` means the work
    /// reached the message bus of the data plane that owns this deployment --
    /// not that it is running, which only the cluster knows and nothing yet
    /// reports back.
    ///
    /// So it moves `Pending` and nothing else. A deployment being deleted is
    /// `Deleting` and must stay there; one that already reached a terminal
    /// state is not walked backwards by a redelivered ack, which matters
    /// because acks are at-least-once by design.
    pub fn hand_off_to_data_plane(&mut self, at: DateTime<Utc>) -> bool {
        if self.status != DeploymentStatus::Pending {
            return false;
        }

        self.status = DeploymentStatus::InProgress;
        self.updated_at = at;
        true
    }

    /// Records that the deployment is serving.
    ///
    /// Returns whether anything changed.
    ///
    /// Reported by the data plane on the operator's own `Running` + `ready`,
    /// which is the only place in the system that knows -- the control plane
    /// hands work over and cannot see what became of it.
    ///
    /// Moves from every state that is still on its way, `Failed` included: an
    /// instance that failed and later came up is serving, and leaving it
    /// `Failed` would be as wrong as never marking it at all. A deletion is not
    /// on its way anywhere, so `Deleting` and `Deleted` do not move.
    pub fn confirm_running(&mut self, at: DateTime<Utc>, observed: Option<Version>) -> bool {
        // An upgrade is not finished because the instance answers. It answers
        // on the old version until the rollout replaces it, and the watcher
        // resyncs every few minutes, so a truthful "running" arrives before
        // the upgrade has done anything at all. Only a version that moved says
        // it landed.
        if self.status == DeploymentStatus::Upgrading {
            return match observed {
                Some(version) if version != self.version => {
                    self.version = version;
                    self.settle_on(DeploymentStatus::Successful, at)
                }
                _ => false,
            };
        }

        // Recorded rather than assumed. What the data plane sees running is
        // the only version anybody can state; an older Genesis sends none, and
        // then what is already recorded stands.
        if let Some(version) = observed {
            self.version = version;
        }

        self.settle_on(DeploymentStatus::Successful, at)
    }

    /// Whether an operation other than the one in flight must wait.
    ///
    /// Only `Upgrading`. A deployment coming up for the first time can still
    /// be deleted -- abandoning something that never worked is a reasonable
    /// thing to want -- and a tear-down already refuses a second one on its
    /// own. An upgrade is different: it is rewriting the instance in place,
    /// and a delete or a resize landing halfway through leaves resources
    /// nobody is tracking.
    ///
    /// Bounded, and that matters: the operator fails an upgrade that has not
    /// come up within its deadline, so this never holds a deployment for
    /// longer than that.
    pub fn is_busy(&self) -> bool {
        self.status == DeploymentStatus::Upgrading
    }

    /// Records that the operator gave up on it.
    ///
    /// Distinct from `fail_hand_off`, which is the control plane failing to
    /// hand the work over at all. This one is the data plane having tried.
    pub fn confirm_failed(&mut self, at: DateTime<Utc>) -> bool {
        self.settle_on(DeploymentStatus::Failed, at)
    }

    /// Moves to the state an outcome reports, when that is meaningful.
    ///
    /// Returns whether anything changed -- and that has to be literally true,
    /// not merely nearly true. The watcher resyncs and reports are
    /// at-least-once, so the same outcome arrives repeatedly by design; a
    /// method that reported "changed" each time would write to the database on
    /// every resync, for every deployment, for ever.
    fn settle_on(&mut self, status: DeploymentStatus, at: DateTime<Utc>) -> bool {
        if !self.is_still_arriving() || self.status == status {
            return false;
        }

        self.status = status;
        self.updated_at = at;
        true
    }

    /// Whether an outcome about this deployment coming up is still meaningful.
    ///
    /// A deployment being torn down is excluded, not because the report would
    /// be wrong, but because it would be about a past that no longer matters --
    /// and marking a deleting deployment `Successful` reads as a deletion that
    /// was undone.
    fn is_still_arriving(&self) -> bool {
        !matches!(
            self.status,
            DeploymentStatus::Deleting | DeploymentStatus::Deleted
        )
    }

    /// Records that the data plane confirmed the resources are gone.
    ///
    /// Returns whether anything changed.
    ///
    /// Only `Deleting` moves. Anything else is a report about a deployment
    /// nobody asked to delete, and acting on it would erase a live deployment
    /// on the strength of a message -- reports are at-least-once and arrive
    /// from outside the control plane, so this refuses rather than trusts.
    ///
    /// Deletion stays soft: the row keeps its `deleted_at` and its history.
    /// What changes is that `Deleting` stops being where deleted deployments
    /// stop for ever.
    pub fn confirm_deletion(&mut self, at: DateTime<Utc>) -> bool {
        if self.status != DeploymentStatus::Deleting {
            return false;
        }

        self.status = DeploymentStatus::Deleted;
        self.updated_at = at;
        true
    }

    /// Records that the control plane could not hand the work over at all.
    ///
    /// Herald reports this when publishing failed, which is a failure the
    /// control plane can see and therefore should show. Same rule as above: it
    /// only moves a deployment that is still waiting.
    pub fn fail_hand_off(&mut self, at: DateTime<Utc>) -> bool {
        if self.status != DeploymentStatus::Pending {
            return false;
        }

        self.status = DeploymentStatus::Failed;
        self.updated_at = at;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deployment(status: DeploymentStatus) -> Deployment {
        let at = Utc::now();
        Deployment {
            id: DeploymentId(Uuid::new_v4()),
            organisation_id: crate::organisation::OrganisationId(Uuid::new_v4()),
            dataplane_id: crate::dataplane::value_objects::DataPlaneId(Uuid::new_v4()),
            name: DeploymentName("auth".to_string()),
            kind: DeploymentKind::Ferriskey,
            version: Version::new(26, 0, 1),
            status,
            namespace: "production-auth".to_string(),
            resources: crate::dataplane::value_objects::DeploymentResources::DEFAULT,
            created_by: crate::user::UserId(Uuid::new_v4()),
            created_at: at,
            updated_at: at,
            deployed_at: None,
            deleted_at: None,
            auto_upgrade: Default::default(),
            maintenance_window: None,
        }
    }

    /// The question the maintainer asked: pods running, deployment stuck in
    /// `in_progress`. This is the transition that was missing.
    #[test]
    fn a_deployment_the_operator_reports_running_becomes_successful() {
        for status in [
            DeploymentStatus::Pending,
            DeploymentStatus::Scheduling,
            DeploymentStatus::InProgress,
        ] {
            let mut subject = deployment(status.clone());

            assert!(subject.confirm_running(Utc::now(), None), "{status:?}");
            assert_eq!(subject.status, DeploymentStatus::Successful);
        }
    }

    /// An instance that failed and later came up is serving. Leaving it
    /// `Failed` would be as wrong as never marking it at all -- and the
    /// operator does retry, so this is a real sequence rather than a
    /// hypothetical one.
    #[test]
    fn a_deployment_that_recovers_stops_being_failed() {
        let mut subject = deployment(DeploymentStatus::Failed);

        assert!(subject.confirm_running(Utc::now(), None));
        assert_eq!(subject.status, DeploymentStatus::Successful);
    }

    /// A deployment being torn down is not on its way anywhere. Marking it
    /// `Successful` would read as a deletion that was undone.
    #[test]
    fn an_outcome_about_coming_up_never_disturbs_a_deletion() {
        for status in [DeploymentStatus::Deleting, DeploymentStatus::Deleted] {
            let mut subject = deployment(status.clone());
            assert!(!subject.confirm_running(Utc::now(), None), "{status:?}");
            assert_eq!(subject.status, status.clone());

            let mut subject = deployment(status.clone());
            assert!(!subject.confirm_failed(Utc::now()), "{status:?}");
            assert_eq!(subject.status, status);
        }
    }

    #[test]
    fn a_deployment_the_operator_gave_up_on_is_marked_failed() {
        let mut subject = deployment(DeploymentStatus::InProgress);

        assert!(subject.confirm_failed(Utc::now()));
        assert_eq!(subject.status, DeploymentStatus::Failed);
    }

    /// Reports are at-least-once and the watcher resyncs, so the same outcome
    /// arrives more than once by design.
    #[test]
    fn reporting_running_twice_changes_nothing_the_second_time() {
        let mut subject = deployment(DeploymentStatus::InProgress);

        assert!(subject.confirm_running(Utc::now(), None));
        assert!(!subject.confirm_running(Utc::now(), None));
        assert_eq!(subject.status, DeploymentStatus::Successful);
    }

    /// The gap this closes: a deletion that completed and one that never got
    /// anywhere both sat in `Deleting`, for ever.
    #[test]
    fn a_confirmed_deletion_leaves_the_deleting_state() {
        let mut subject = deployment(DeploymentStatus::Deleting);

        assert!(subject.confirm_deletion(Utc::now()));
        assert_eq!(subject.status, DeploymentStatus::Deleted);
    }

    /// Reports arrive from outside the control plane and are at-least-once, so
    /// a confirmation for a deployment nobody asked to delete must not erase a
    /// live one.
    #[test]
    fn a_deletion_report_never_deletes_a_deployment_nobody_asked_to_delete() {
        for status in [
            DeploymentStatus::Pending,
            DeploymentStatus::InProgress,
            DeploymentStatus::Successful,
            DeploymentStatus::Failed,
        ] {
            let mut subject = deployment(status.clone());

            assert!(!subject.confirm_deletion(Utc::now()), "{status:?}");
            assert_eq!(subject.status, status);
        }
    }

    /// Redelivery is normal, so confirming twice must be inert rather than
    /// merely harmless-looking.
    #[test]
    fn confirming_a_deletion_twice_changes_nothing_the_second_time() {
        let mut subject = deployment(DeploymentStatus::Deleting);

        assert!(subject.confirm_deletion(Utc::now()));
        assert!(!subject.confirm_deletion(Utc::now()));
        assert_eq!(subject.status, DeploymentStatus::Deleted);
    }

    #[test]
    fn deleted_round_trips_through_its_string_form() {
        assert_eq!(DeploymentStatus::Deleted.to_string(), "deleted");
        assert!(matches!(
            DeploymentStatus::try_from("deleted"),
            Ok(DeploymentStatus::Deleted)
        ));
    }

    /// The one transition the control plane can make on its own evidence: an
    /// ack of `published` means the work reached the data plane's bus.
    #[test]
    fn a_pending_deployment_moves_to_in_progress_when_the_work_is_handed_over() {
        let mut deployment = deployment(DeploymentStatus::Pending);

        assert!(deployment.hand_off_to_data_plane(Utc::now()));
        assert_eq!(deployment.status, DeploymentStatus::InProgress);
    }

    /// Acks are at-least-once by design, so a redelivered one must not walk a
    /// deployment backwards out of a state it has already reached.
    #[test]
    fn handing_over_again_changes_nothing() {
        for status in [
            DeploymentStatus::InProgress,
            DeploymentStatus::Successful,
            DeploymentStatus::Failed,
        ] {
            let mut subject = deployment(status.clone());

            assert!(!subject.hand_off_to_data_plane(Utc::now()), "{status:?}");
            assert_eq!(subject.status, status);
        }
    }

    /// A deployment being deleted is `Deleting` and stays there. Its delete
    /// action is handed over the same way a create is, and promoting it to
    /// `InProgress` would report a deletion as a deployment starting.
    #[test]
    fn handing_over_a_deletion_does_not_report_it_as_starting() {
        let mut deployment = deployment(DeploymentStatus::Deleting);

        assert!(!deployment.hand_off_to_data_plane(Utc::now()));
        assert_eq!(deployment.status, DeploymentStatus::Deleting);
    }

    #[test]
    fn a_hand_off_that_failed_is_reported_as_failed() {
        let mut deployment = deployment(DeploymentStatus::Pending);

        assert!(deployment.fail_hand_off(Utc::now()));
        assert_eq!(deployment.status, DeploymentStatus::Failed);
    }

    #[test]
    fn a_failed_hand_off_never_overrides_a_deletion() {
        let mut deployment = deployment(DeploymentStatus::Deleting);

        assert!(!deployment.fail_hand_off(Utc::now()));
        assert_eq!(deployment.status, DeploymentStatus::Deleting);
    }

    #[test]
    fn deployment_kind_display_and_parse() {
        assert_eq!(DeploymentKind::Ferriskey.to_string(), "ferriskey");
        assert_eq!(DeploymentKind::Keycloak.to_string(), "keycloak");

        assert!(matches!(
            DeploymentKind::try_from("ferriskey"),
            Ok(DeploymentKind::Ferriskey)
        ));
        assert!(matches!(
            DeploymentKind::try_from("KEYCLOAK"),
            Ok(DeploymentKind::Keycloak)
        ));
    }

    #[test]
    fn deployment_status_display_and_parse() {
        assert_eq!(DeploymentStatus::Pending.to_string(), "pending");
        assert_eq!(
            DeploymentStatus::UpgradeRequired.to_string(),
            "upgrade_required"
        );
        assert_eq!(DeploymentStatus::Deleting.to_string(), "deleting");

        assert!(matches!(
            DeploymentStatus::try_from("in_progress"),
            Ok(DeploymentStatus::InProgress)
        ));
        assert!(matches!(
            DeploymentStatus::try_from("FAILED"),
            Ok(DeploymentStatus::Failed)
        ));
        assert!(matches!(
            DeploymentStatus::try_from("DELETING"),
            Ok(DeploymentStatus::Deleting)
        ));
    }

    #[test]
    fn deployment_id_from_str() {
        let id = Uuid::new_v4();
        let parsed = DeploymentId::from_str(&id.to_string()).unwrap();

        assert_eq!(parsed.0, id);
    }

    /// The race this rule exists for. An instance being upgraded keeps
    /// answering on the version it already runs, and the watcher resyncs every
    /// few minutes, so a truthful "running" arrives long before the rollout
    /// has replaced anything.
    #[test]
    fn an_upgrade_is_not_finished_because_the_instance_still_answers() {
        let mut subject = deployment(DeploymentStatus::Upgrading);
        subject.version = Version::new(26, 0, 0);

        let moved = subject.confirm_running(Utc::now(), Some(Version::new(26, 0, 0)));

        assert!(
            !moved,
            "the old version reported as running is not progress"
        );
        assert_eq!(subject.status, DeploymentStatus::Upgrading);
        assert_eq!(subject.version, Version::new(26, 0, 0));
    }

    /// And the other half: a version that moved is the only thing that says it
    /// landed, so it settles the deployment and is recorded.
    #[test]
    fn an_upgrade_finishes_when_the_version_that_answers_has_moved() {
        let mut subject = deployment(DeploymentStatus::Upgrading);
        subject.version = Version::new(26, 0, 0);

        let moved = subject.confirm_running(Utc::now(), Some(Version::new(26, 0, 1)));

        assert!(moved);
        assert_eq!(subject.status, DeploymentStatus::Successful);
        assert_eq!(subject.version, Version::new(26, 0, 1));
    }

    /// A data plane built before the version travelled sends none. Settling on
    /// that would mark an upgrade complete with no evidence at all.
    #[test]
    fn an_upgrade_reported_without_a_version_stays_upgrading() {
        let mut subject = deployment(DeploymentStatus::Upgrading);

        assert!(!subject.confirm_running(Utc::now(), None));
        assert_eq!(subject.status, DeploymentStatus::Upgrading);
    }

    /// The ordinary path is unchanged: a deployment coming up for the first
    /// time settles on the version it reports, which is the one it was created
    /// with.
    #[test]
    fn a_deployment_coming_up_records_the_version_that_answers() {
        let mut subject = deployment(DeploymentStatus::InProgress);
        subject.version = Version::new(26, 0, 0);

        assert!(subject.confirm_running(Utc::now(), Some(Version::new(26, 0, 0))));
        assert_eq!(subject.status, DeploymentStatus::Successful);
        assert_eq!(subject.version, Version::new(26, 0, 0));
    }

    /// What is recorded is what was seen. If an instance comes up on something
    /// other than what the row says, the row is what is wrong.
    #[test]
    fn a_deployment_records_whatever_version_is_actually_running() {
        let mut subject = deployment(DeploymentStatus::InProgress);
        subject.version = Version::new(26, 0, 0);

        assert!(subject.confirm_running(Utc::now(), Some(Version::new(26, 1, 0))));
        assert_eq!(subject.version, Version::new(26, 1, 0));
    }
}
