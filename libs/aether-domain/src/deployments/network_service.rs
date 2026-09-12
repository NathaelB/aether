//! Reading and replacing who may reach a deployment.
//!
//! Its own service rather than two more methods on the deployment one,
//! because it is the only part of a deployment where reading and changing are
//! different rights, and the gate belongs next to the thing it gates.

use aether_auth::Identity;

use crate::{
    CoreError,
    deployments::{
        Deployment, DeploymentId,
        network::NetworkAccess,
        ports::{DeploymentRepository, NetworkAccessPolicy, NetworkAccessService},
    },
    organisation::OrganisationId,
};

pub struct NetworkAccessServiceImpl<R, P> {
    deployment_repository: R,
    policy: P,
}

impl<R, P> NetworkAccessServiceImpl<R, P> {
    pub fn new(deployment_repository: R, policy: P) -> Self {
        Self {
            deployment_repository,
            policy,
        }
    }
}

impl<R, P> NetworkAccessServiceImpl<R, P>
where
    R: DeploymentRepository,
{
    /// Reads a deployment, or says it does not exist.
    ///
    /// Scoped by organisation, and a deployment belonging to somebody else is
    /// not found rather than forbidden: answering differently would tell a
    /// caller which ids exist in organisations they cannot see.
    async fn deployment(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<Deployment, CoreError> {
        self.deployment_repository
            .get_by_id(deployment_id)
            .await?
            .filter(|deployment| deployment.organisation_id == organisation_id)
            .filter(|deployment| deployment.deleted_at.is_none())
            .ok_or(CoreError::DeploymentNotFound {
                id: deployment_id.0,
            })
    }
}

impl<R, P> NetworkAccessService for NetworkAccessServiceImpl<R, P>
where
    R: DeploymentRepository,
    P: NetworkAccessPolicy,
{
    async fn network_access(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<NetworkAccess, CoreError> {
        self.policy
            .can_view_network_access(identity, organisation_id)
            .await?;

        Ok(self
            .deployment(organisation_id, deployment_id)
            .await?
            .network_access)
    }

    async fn set_network_access(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
        access: NetworkAccess,
    ) -> Result<Deployment, CoreError> {
        // Checked before the deployment is read, so a caller who may not
        // change anything cannot use the difference between 403 and 404 to
        // find out which deployments exist.
        self.policy
            .can_change_network_access(identity, organisation_id)
            .await?;

        let mut deployment = self.deployment(organisation_id, deployment_id).await?;

        deployment.network_access = access;
        deployment.updated_at = chrono::Utc::now();
        self.deployment_repository
            .update(deployment.clone())
            .await?;

        Ok(deployment)
    }
}
