use crate::{deployments::DeploymentId, organisation::OrganisationId, version::Version};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestUpgradeCommand {
    pub organisation_id: OrganisationId,
    pub deployment_id: DeploymentId,
    pub target: Version,
}
