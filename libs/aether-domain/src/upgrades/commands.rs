use crate::{
    deployments::DeploymentId,
    organisation::OrganisationId,
    upgrades::policy::{AutoUpgradePolicy, MaintenanceWindow},
    version::Version,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestUpgradeCommand {
    pub organisation_id: OrganisationId,
    pub deployment_id: DeploymentId,
    pub target: Version,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetUpgradeSettingsCommand {
    pub organisation_id: OrganisationId,
    pub deployment_id: DeploymentId,
    pub auto_upgrade: AutoUpgradePolicy,
    pub maintenance_window: Option<MaintenanceWindow>,
}
