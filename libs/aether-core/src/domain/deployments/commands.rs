use crate::{organisation::OrganisationId, user::UserId};

use super::{DeploymentKind, DeploymentName, DeploymentStatus, DeploymentVersion};

/// Command to create a new deployment
#[derive(Debug, Clone)]
pub struct CreateDeploymentCommand {
    pub organisation_id: OrganisationId,
    pub name: DeploymentName,
    pub kind: DeploymentKind,
    pub version: DeploymentVersion,
    pub status: DeploymentStatus,
    pub namespace: String,
    pub created_by: UserId,
}

impl CreateDeploymentCommand {
    pub fn new(
        organisation_id: OrganisationId,
        name: DeploymentName,
        kind: DeploymentKind,
        version: DeploymentVersion,
        status: DeploymentStatus,
        namespace: String,
        created_by: UserId,
    ) -> Self {
        Self {
            organisation_id,
            name,
            kind,
            version,
            status,
            namespace,
            created_by,
        }
    }
}

/// Command to update an existing deployment
#[derive(Debug, Clone, Default)]
pub struct UpdateDeploymentCommand {
    pub name: Option<DeploymentName>,
    pub kind: Option<DeploymentKind>,
    pub version: Option<DeploymentVersion>,
    pub status: Option<DeploymentStatus>,
    pub namespace: Option<String>,
    pub deployed_at: Option<Option<chrono::DateTime<chrono::Utc>>>,
    pub deleted_at: Option<Option<chrono::DateTime<chrono::Utc>>>,
}

impl UpdateDeploymentCommand {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_name(mut self, name: DeploymentName) -> Self {
        self.name = Some(name);
        self
    }

    pub fn with_kind(mut self, kind: DeploymentKind) -> Self {
        self.kind = Some(kind);
        self
    }

    pub fn with_version(mut self, version: DeploymentVersion) -> Self {
        self.version = Some(version);
        self
    }

    pub fn with_status(mut self, status: DeploymentStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub fn with_namespace(mut self, namespace: String) -> Self {
        self.namespace = Some(namespace);
        self
    }

    pub fn with_deployed_at(mut self, deployed_at: Option<chrono::DateTime<chrono::Utc>>) -> Self {
        self.deployed_at = Some(deployed_at);
        self
    }

    pub fn with_deleted_at(mut self, deleted_at: Option<chrono::DateTime<chrono::Utc>>) -> Self {
        self.deleted_at = Some(deleted_at);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.kind.is_none()
            && self.version.is_none()
            && self.status.is_none()
            && self.namespace.is_none()
            && self.deployed_at.is_none()
            && self.deleted_at.is_none()
    }
}
