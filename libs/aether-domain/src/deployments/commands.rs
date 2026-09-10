use crate::{
    dataplane::value_objects::{DataPlaneId, DataPlaneMode, DeploymentResources, Region},
    organisation::OrganisationId,
    user::UserId,
};

use super::{DeploymentKind, DeploymentName, DeploymentStatus};
use crate::version::Version;

/// Command to create a new deployment
#[derive(Debug, Clone)]
pub struct CreateDeploymentCommand {
    pub organisation_id: OrganisationId,
    pub name: DeploymentName,
    pub kind: DeploymentKind,
    pub version: Version,
    pub status: DeploymentStatus,
    pub namespace: String,
    pub created_by: UserId,
    /// Where the caller wants this to run. Carried rather than assumed: a
    /// region silently substituted for another is a deployment in the wrong
    /// jurisdiction.
    pub region: Region,
    /// Whether the caller wants a data plane of their own.
    pub mode: DataPlaneMode,
    /// How big it should be. Defaults are applied at the API boundary, so by
    /// the time a command exists the size is explicit.
    pub resources: DeploymentResources,
}

impl CreateDeploymentCommand {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        organisation_id: OrganisationId,
        name: DeploymentName,
        kind: DeploymentKind,
        version: Version,
        status: DeploymentStatus,
        namespace: String,
        created_by: UserId,
        region: Region,
        mode: DataPlaneMode,
        resources: DeploymentResources,
    ) -> Self {
        Self {
            organisation_id,
            name,
            kind,
            version,
            status,
            namespace,
            created_by,
            region,
            mode,
            resources,
        }
    }
}

/// Command to update an existing deployment
#[derive(Debug, Clone, Default)]
pub struct UpdateDeploymentCommand {
    pub name: Option<DeploymentName>,
    pub kind: Option<DeploymentKind>,
    pub version: Option<Version>,
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

    pub fn with_version(mut self, version: Version) -> Self {
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
            Version::new(1, 0, 0),
            DeploymentStatus::Pending,
            "namespace".to_string(),
            UserId(Uuid::new_v4()),
            Region::new("fr-par"),
            DataPlaneMode::Shared,
            DeploymentResources::DEFAULT,
        );

        assert_eq!(command.name.0, "app");
        assert_eq!(command.kind, DeploymentKind::Keycloak);
        assert_eq!(command.version.to_string(), "1.0.0");
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
            .with_version(Version::new(2, 0, 0))
            .with_status(DeploymentStatus::Successful)
            .with_namespace("new-namespace".to_string())
            .with_deployed_at(Some(deployed_at))
            .with_deleted_at(Some(deleted_at));

        assert_eq!(command.name.unwrap().0, "new");
        assert_eq!(command.kind.unwrap(), DeploymentKind::Ferriskey);
        assert_eq!(command.version.unwrap().to_string(), "2.0.0");
        assert_eq!(command.status.unwrap(), DeploymentStatus::Successful);
        assert_eq!(command.namespace.unwrap(), "new-namespace");
        assert_eq!(command.deployed_at.unwrap(), Some(deployed_at));
        assert_eq!(command.deleted_at.unwrap(), Some(deleted_at));
    }
}

/// What a data plane observed happening to a deployment it was given.
///
/// Parsed at the boundary rather than carried as a string, so an outcome the
/// control plane does not understand is rejected where the caller can be told
/// about it, instead of reaching a match arm that quietly does nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentOutcome {
    /// The resources are gone. Reported by the component that removed them.
    Deleted,
    /// The deployment is serving. Reported on the operator's `Running` +
    /// `ready`, which is the only place that knows.
    Running,
    /// The operator gave up on it. An observation, not a timeout.
    Failed,
}

#[derive(Debug, Clone)]
pub struct ReportDeploymentOutcomeCommand {
    pub dataplane_id: DataPlaneId,
    pub deployment_id: super::DeploymentId,
    pub outcome: DeploymentOutcome,
}

impl ReportDeploymentOutcomeCommand {
    pub fn parse(
        dataplane_id: DataPlaneId,
        deployment_id: super::DeploymentId,
        outcome: &str,
    ) -> Result<Self, String> {
        let outcome = match outcome {
            "deleted" => DeploymentOutcome::Deleted,
            "running" => DeploymentOutcome::Running,
            "failed" => DeploymentOutcome::Failed,
            other => {
                return Err(format!(
                    "unknown deployment outcome '{other}', expected one of: deleted, running, failed"
                ));
            }
        };

        Ok(Self {
            dataplane_id,
            deployment_id,
            outcome,
        })
    }
}
