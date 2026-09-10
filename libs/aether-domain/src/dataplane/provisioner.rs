use crate::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        value_objects::{DataPlaneId, DeploymentResources, Region},
    },
    organisation::OrganisationId,
};

/// Creates the infrastructure a data plane runs on.
///
/// This is the one place the pull model reverses. Everywhere else the data
/// plane claims work from the control plane; here the control plane must act
/// on a cluster that does not exist yet, because an empty cluster has no
/// Herald in it to do the claiming.
#[cfg_attr(test, mockall::automock)]
pub trait ClusterProvisioner: Send + Sync {
    fn provision(
        &self,
        request: ProvisionRequest,
    ) -> impl Future<Output = Result<DataPlane, CoreError>> + Send;

    /// Releases the infrastructure. Must be idempotent: it is called on
    /// cleanup paths that cannot know whether a previous attempt got through.
    fn deprovision(&self, id: &DataPlaneId) -> impl Future<Output = Result<(), CoreError>> + Send;
}

pub struct ProvisionRequest {
    pub organisation_id: OrganisationId,
    pub region: Region,
    /// What the first deployment needs. A provisioner sizes the cluster to at
    /// least this, and is free to size it larger.
    pub minimum: DeploymentResources,
}
