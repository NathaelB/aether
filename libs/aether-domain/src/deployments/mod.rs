use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{CoreError, organisation::OrganisationId, user::UserId};

pub mod commands;
pub mod ports;
pub mod service;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, ToSchema)]
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
    pub name: DeploymentName,

    pub kind: DeploymentKind,
    pub version: DeploymentVersion,

    pub status: DeploymentStatus,
    pub namespace: String,

    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub deployed_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deployment_kind_display_and_parse() {
        assert_eq!(DeploymentKind::Ferriskey.to_string(), "ferris_key");
        assert_eq!(DeploymentKind::Keycloak.to_string(), "keycloak");

        assert!(matches!(
            DeploymentKind::try_from("ferris_key"),
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
