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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn create_deployment_command_sets_fields() {
        let command = CreateDeploymentCommand::new(
            OrganisationId(Uuid::new_v4()),
            DeploymentName("app".to_string()),
            DeploymentKind::Keycloak,
            DeploymentVersion("1.0.0".to_string()),
            DeploymentStatus::Pending,
            "namespace".to_string(),
            UserId(Uuid::new_v4()),
        );

        assert_eq!(command.name.0, "app");
        assert_eq!(command.kind, DeploymentKind::Keycloak);
        assert_eq!(command.version.0, "1.0.0");
        assert_eq!(command.status, DeploymentStatus::Pending);
        assert_eq!(command.namespace, "namespace");
    }

    #[test]
    fn update_deployment_command_is_empty_when_no_fields_set() {
        let command = UpdateDeploymentCommand::new();

        assert!(command.is_empty());
    }

    #[test]
    fn update_deployment_command_builder_sets_fields() {
        let deployed_at = Utc::now();
        let deleted_at = Utc::now();

        let command = UpdateDeploymentCommand::new()
            .with_name(DeploymentName("new".to_string()))
            .with_kind(DeploymentKind::Ferriskey)
            .with_version(DeploymentVersion("2.0.0".to_string()))
            .with_status(DeploymentStatus::Successful)
            .with_namespace("new-namespace".to_string())
            .with_deployed_at(Some(deployed_at))
            .with_deleted_at(Some(deleted_at));

        assert_eq!(command.name.unwrap().0, "new");
        assert_eq!(command.kind.unwrap(), DeploymentKind::Ferriskey);
        assert_eq!(command.version.unwrap().0, "2.0.0");
        assert_eq!(command.status.unwrap(), DeploymentStatus::Successful);
        assert_eq!(command.namespace.unwrap(), "new-namespace");
        assert_eq!(command.deployed_at.unwrap(), Some(deployed_at));
        assert_eq!(command.deleted_at.unwrap(), Some(deleted_at));
    }
}
