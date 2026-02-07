use aether_auth::Identity;
use aether_domain::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        ports::DataPlaneService,
        service::DataPlaneServiceImpl,
        value_objects::{CreateDataplaneCommand, DataPlaneId},
    },
    deployments::Deployment,
};
use aether_postgres::deployments::PostgresDeploymentRepository;

use crate::{AetherService, infrastructure::dataplane::PostgresDataPlaneRepository};

impl DataPlaneService for AetherService {
    async fn create_dataplane(
        &self,
        identity: Identity,
        command: CreateDataplaneCommand,
    ) -> Result<DataPlane, CoreError> {
        let tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;
        let tx = tokio::sync::Mutex::new(Some(tx));

        let result = {
            let dataplane_repository = PostgresDataPlaneRepository::from_tx(&tx);
            let deployment_repository = PostgresDeploymentRepository::from_tx(&tx);
            let dataplane_service =
                DataPlaneServiceImpl::new(dataplane_repository, deployment_repository);

            dataplane_service.create_dataplane(identity, command).await
        };

        match result {
            Ok(dataplane) => {
                super::take_transaction(&tx)
                    .await?
                    .commit()
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                Ok(dataplane)
            }
            Err(err) => {
                super::take_transaction(&tx)
                    .await?
                    .rollback()
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                Err(err)
            }
        }
    }

    async fn list_dataplanes(&self, identity: Identity) -> Result<Vec<DataPlane>, CoreError> {
        let dataplane_repository = PostgresDataPlaneRepository::from_pool(self.pool());
        let deployment_repository = PostgresDeploymentRepository::from_pool(self.pool());
        let dataplane_service =
            DataPlaneServiceImpl::new(dataplane_repository, deployment_repository);

        dataplane_service.list_dataplanes(identity).await
    }

    async fn get_dataplane(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
    ) -> Result<DataPlane, CoreError> {
        let dataplane_repository = PostgresDataPlaneRepository::from_pool(self.pool());
        let deployment_repository = PostgresDeploymentRepository::from_pool(self.pool());
        let dataplane_service =
            DataPlaneServiceImpl::new(dataplane_repository, deployment_repository);

        dataplane_service
            .get_dataplane(identity, dataplane_id)
            .await
    }

    async fn get_deployments_in_dataplane(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
    ) -> Result<Vec<Deployment>, CoreError> {
        let dataplane_repository = PostgresDataPlaneRepository::from_pool(self.pool());
        let deployment_repository = PostgresDeploymentRepository::from_pool(self.pool());
        let dataplane_service =
            DataPlaneServiceImpl::new(dataplane_repository, deployment_repository);

        dataplane_service
            .get_deployments_in_dataplane(identity, dataplane_id)
            .await
    }
}
