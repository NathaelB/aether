use aether_auth::Identity;
use aether_domain::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        ports::DataPlaneService,
        service::DataPlaneServiceImpl,
        value_objects::{CreateDataplaneCommand, DataPlaneId, ListDataPlaneDeploymentsCommand},
    },
    deployments::Deployment,
};
use aether_macros::transactional;

use crate::AetherService;

impl DataPlaneService for AetherService {
    #[transactional(data_plane, deployment)]
    async fn create_dataplane(
        &self,
        identity: Identity,
        command: CreateDataplaneCommand,
    ) -> Result<DataPlane, CoreError> {
        DataPlaneServiceImpl::new(data_plane_repository, deployment_repository)
            .create_dataplane(identity, command)
            .await
    }

    #[transactional(data_plane, deployment)]
    async fn list_dataplanes(&self, identity: Identity) -> Result<Vec<DataPlane>, CoreError> {
        DataPlaneServiceImpl::new(data_plane_repository, deployment_repository)
            .list_dataplanes(identity)
            .await
    }

    #[transactional(data_plane, deployment)]
    async fn get_dataplane(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
    ) -> Result<DataPlane, CoreError> {
        DataPlaneServiceImpl::new(data_plane_repository, deployment_repository)
            .get_dataplane(identity, dataplane_id)
            .await
    }

    #[transactional(data_plane, deployment)]
    async fn get_deployments_in_dataplane(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
        command: ListDataPlaneDeploymentsCommand,
    ) -> Result<Vec<Deployment>, CoreError> {
        DataPlaneServiceImpl::new(data_plane_repository, deployment_repository)
            .get_deployments_in_dataplane(identity, dataplane_id, command)
            .await
    }
}
