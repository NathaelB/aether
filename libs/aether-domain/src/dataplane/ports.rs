use aether_auth::Identity;

use crate::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        value_objects::{
            CreateDataplaneCommand, DataPlaneId, ListDataPlaneDeploymentsCommand, Region,
        },
    },
    deployments::Deployment,
};

pub trait DataPlaneService: Send + Sync {
    fn create_dataplane(
        &self,
        identity: Identity,
        command: CreateDataplaneCommand,
    ) -> impl Future<Output = Result<DataPlane, CoreError>> + Send;
    fn list_dataplanes(
        &self,
        identity: Identity,
    ) -> impl Future<Output = Result<Vec<DataPlane>, CoreError>> + Send;
    fn get_dataplane(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
    ) -> impl Future<Output = Result<DataPlane, CoreError>> + Send;
    fn get_deployments_in_dataplane(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
        command: ListDataPlaneDeploymentsCommand,
    ) -> impl Future<Output = Result<Vec<Deployment>, CoreError>> + Send;
}

#[cfg_attr(test, mockall::automock)]
pub trait DataPlaneRepository: Send + Sync {
    fn find_by_id(
        &self,
        id: &DataPlaneId,
    ) -> impl Future<Output = Result<Option<DataPlane>, CoreError>> + Send;
    fn find_active_shared_by_region(
        &self,
        region: &Region,
    ) -> impl Future<Output = Result<Vec<DataPlane>, CoreError>> + Send;
    fn find_available(
        &self,
        region: Option<Region>,
        required_capacity: u32,
    ) -> impl Future<Output = Result<Option<DataPlane>, CoreError>> + Send;
    fn list_all(&self) -> impl Future<Output = Result<Vec<DataPlane>, CoreError>> + Send;
    fn current_load(&self, id: &DataPlaneId)
    -> impl Future<Output = Result<u32, CoreError>> + Send;
    fn save(&self, dataplane: &DataPlane) -> impl Future<Output = Result<(), CoreError>> + Send;
}
