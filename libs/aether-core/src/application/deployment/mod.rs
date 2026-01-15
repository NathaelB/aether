use crate::{
    AetherService, CoreError,
    deployments::{
        Deployment, DeploymentId,
        commands::{CreateDeploymentCommand, UpdateDeploymentCommand},
        ports::DeploymentService,
        service::DeploymentServiceImpl,
    },
    infrastructure::deployments::PostgresDeploymentRepository,
    organisation::OrganisationId,
};

impl DeploymentService for AetherService {
    async fn create_deployment(
        &self,
        command: CreateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        let tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;
        let tx = tokio::sync::Mutex::new(Some(tx));

        let result = {
            let deployment_repository = PostgresDeploymentRepository::from_tx(&tx);
            let deployment_service = DeploymentServiceImpl::new(deployment_repository);

            deployment_service.create_deployment(command).await
        };

        match result {
            Ok(deployment) => {
                super::take_transaction(&tx)
                    .await?
                    .commit()
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                Ok(deployment)
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

    async fn delete_deployment(&self, deployment_id: DeploymentId) -> Result<(), CoreError> {
        let tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;
        let tx = tokio::sync::Mutex::new(Some(tx));

        let result = {
            let deployment_repository = PostgresDeploymentRepository::from_tx(&tx);
            let deployment_service = DeploymentServiceImpl::new(deployment_repository);

            deployment_service.delete_deployment(deployment_id).await
        };

        match result {
            Ok(()) => {
                super::take_transaction(&tx)
                    .await?
                    .commit()
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                Ok(())
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

    async fn delete_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<(), CoreError> {
        let tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;
        let tx = tokio::sync::Mutex::new(Some(tx));

        let result = {
            let deployment_repository = PostgresDeploymentRepository::from_tx(&tx);
            let deployment_service = DeploymentServiceImpl::new(deployment_repository);

            deployment_service
                .delete_deployment_for_organisation(organisation_id, deployment_id)
                .await
        };

        match result {
            Ok(()) => {
                super::take_transaction(&tx)
                    .await?
                    .commit()
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                Ok(())
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

    async fn get_deployment(
        &self,
        deployment_id: DeploymentId,
    ) -> Result<Option<Deployment>, CoreError> {
        let deployment_repository = PostgresDeploymentRepository::from_pool(self.pool());
        let deployment_service = DeploymentServiceImpl::new(deployment_repository);

        deployment_service.get_deployment(deployment_id).await
    }

    async fn get_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<Deployment, CoreError> {
        let deployment_repository = PostgresDeploymentRepository::from_pool(self.pool());
        let deployment_service = DeploymentServiceImpl::new(deployment_repository);

        deployment_service
            .get_deployment_for_organisation(organisation_id, deployment_id)
            .await
    }

    async fn list_deployments_by_organisation(
        &self,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Deployment>, CoreError> {
        let deployment_repository = PostgresDeploymentRepository::from_pool(self.pool());
        let deployment_service = DeploymentServiceImpl::new(deployment_repository);

        deployment_service
            .list_deployments_by_organisation(organisation_id)
            .await
    }

    async fn update_deployment(
        &self,
        deployment_id: DeploymentId,
        command: UpdateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        let tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;
        let tx = tokio::sync::Mutex::new(Some(tx));

        let result = {
            let deployment_repository = PostgresDeploymentRepository::from_tx(&tx);
            let deployment_service = DeploymentServiceImpl::new(deployment_repository);

            deployment_service
                .update_deployment(deployment_id, command)
                .await
        };

        match result {
            Ok(deployment) => {
                super::take_transaction(&tx)
                    .await?
                    .commit()
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                Ok(deployment)
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

    async fn update_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
        command: UpdateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        let tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;
        let tx = tokio::sync::Mutex::new(Some(tx));

        let result = {
            let deployment_repository = PostgresDeploymentRepository::from_tx(&tx);
            let deployment_service = DeploymentServiceImpl::new(deployment_repository);

            deployment_service
                .update_deployment_for_organisation(organisation_id, deployment_id, command)
                .await
        };

        match result {
            Ok(deployment) => {
                super::take_transaction(&tx)
                    .await?
                    .commit()
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                Ok(deployment)
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
}
