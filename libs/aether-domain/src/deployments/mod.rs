use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::dataplane::value_objects::DeploymentResources;
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, ToSchema)]
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
    Deleting,
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
            _ => Err(CoreError::InternalError(format!(
                "Invalid deployment status: {}",
                value
            ))),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, ToSchema)]
pub struct DeploymentVersion(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Deployment {
    pub id: DeploymentId,
    pub organisation_id: OrganisationId,
    pub dataplane_id: DataPlaneId,
    pub name: DeploymentName,

    pub kind: DeploymentKind,
    pub version: DeploymentVersion,

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
            version: DeploymentVersion("latest".to_string()),
            status,
            namespace: "production-auth".to_string(),
            resources: crate::dataplane::value_objects::DeploymentResources::DEFAULT,
            created_by: crate::user::UserId(Uuid::new_v4()),
            created_at: at,
            updated_at: at,
            deployed_at: None,
            deleted_at: None,
        }
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
}
