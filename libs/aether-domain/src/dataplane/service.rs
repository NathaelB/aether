use aether_auth::Identity;

use crate::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        ports::{DataPlaneRepository, DataPlaneService},
        value_objects::{CreateDataplaneCommand, DataPlaneId},
    },
    deployments::{Deployment, ports::DeploymentRepository},
};

#[derive(Debug)]
pub struct DataPlaneServiceImpl<DP, D>
where
    DP: DataPlaneRepository,
    D: DeploymentRepository,
{
    dataplane_repository: DP,
    deployment_repository: D,
}

impl<DP, D> DataPlaneServiceImpl<DP, D>
where
    DP: DataPlaneRepository,
    D: DeploymentRepository,
{
    pub fn new(dataplane_repository: DP, deployment_repository: D) -> Self {
        Self {
            dataplane_repository,
            deployment_repository,
        }
    }
}

impl<DP, D> DataPlaneService for DataPlaneServiceImpl<DP, D>
where
    DP: DataPlaneRepository,
    D: DeploymentRepository,
{
    async fn create_dataplane(
        &self,
        identity: Identity,
        command: CreateDataplaneCommand,
    ) -> Result<DataPlane, CoreError> {
        let dataplane = DataPlane::new(command.mode, command.region, command.capacity);
        self.dataplane_repository.save(&dataplane).await?;

        Ok(dataplane)
    }

    async fn list_dataplanes(&self, _identity: Identity) -> Result<Vec<DataPlane>, CoreError> {
        self.dataplane_repository.list_all().await
    }

    async fn get_dataplane(
        &self,
        _identity: Identity,
        dataplane_id: DataPlaneId,
    ) -> Result<DataPlane, CoreError> {
        let dataplane = self
            .dataplane_repository
            .find_by_id(&dataplane_id)
            .await?
            .ok_or(CoreError::DataPlaneNotFound { id: dataplane_id })?;

        Ok(dataplane)
    }

    async fn get_deployments_in_dataplane(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
    ) -> Result<Vec<Deployment>, CoreError> {
        let deployments = self
            .deployment_repository
            .list_by_dataplane(&dataplane_id)
            .await?;

        Ok(deployments)
    }
}
