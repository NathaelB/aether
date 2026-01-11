use std::sync::Arc;

use crate::domain::{
    CoreError,
    deployments::{
        Deployment, DeploymentId,
        commands::{CreateDeploymentCommand, UpdateDeploymentCommand},
        ports::{DeploymentRepository, DeploymentService},
    },
    organisation::OrganisationId,
};

#[derive(Clone, Debug)]
pub struct DeploymentServiceImpl<D>
where
    D: DeploymentRepository,
{
    deployment_repository: Arc<D>,
}

impl<D> DeploymentServiceImpl<D>
where
    D: DeploymentRepository,
{
    pub fn new(deployment_repository: Arc<D>) -> Self {
        Self {
            deployment_repository,
        }
    }
}

impl<D> DeploymentService for DeploymentServiceImpl<D>
where
    D: DeploymentRepository,
{
    async fn create_deployment(
        &self,
        command: CreateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        let now = chrono::Utc::now();
        let deployment = Deployment {
            id: DeploymentId(uuid::Uuid::new_v4()),
            organisation_id: command.organisation_id,
            name: command.name,
            kind: command.kind,
            version: command.version,
            status: command.status,
            namespace: command.namespace,
            created_by: command.created_by,
            created_at: now,
            updated_at: now,
            deployed_at: None,
            deleted_at: None,
        };

        self.deployment_repository.insert(deployment.clone()).await?;
        Ok(deployment)
    }

    async fn get_deployment(
        &self,
        deployment_id: DeploymentId,
    ) -> Result<Option<Deployment>, CoreError> {
        self.deployment_repository.get_by_id(deployment_id).await
    }

    async fn get_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<Deployment, CoreError> {
        let deployment = self
            .deployment_repository
            .get_by_id(deployment_id)
            .await?
            .ok_or(CoreError::InternalError("Deployment not found".to_string()))?;

        if deployment.organisation_id != organisation_id {
            return Err(CoreError::InternalError("Deployment not found".to_string()));
        }

        Ok(deployment)
    }

    async fn list_deployments_by_organisation(
        &self,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Deployment>, CoreError> {
        self.deployment_repository
            .list_by_organisation(organisation_id)
            .await
    }

    async fn update_deployment(
        &self,
        deployment_id: DeploymentId,
        command: UpdateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        if command.is_empty() {
            return Err(CoreError::InternalError(
                "Update command cannot be empty".to_string(),
            ));
        }

        let mut deployment = self
            .deployment_repository
            .get_by_id(deployment_id)
            .await?
            .ok_or(CoreError::InternalError("Deployment not found".to_string()))?;

        if let Some(name) = command.name {
            deployment.name = name;
        }
        if let Some(kind) = command.kind {
            deployment.kind = kind;
        }
        if let Some(version) = command.version {
            deployment.version = version;
        }
        if let Some(status) = command.status {
            deployment.status = status;
        }
        if let Some(namespace) = command.namespace {
            deployment.namespace = namespace;
        }
        if let Some(deployed_at) = command.deployed_at {
            deployment.deployed_at = deployed_at;
        }
        if let Some(deleted_at) = command.deleted_at {
            deployment.deleted_at = deleted_at;
        }

        deployment.updated_at = chrono::Utc::now();

        self.deployment_repository.update(deployment.clone()).await?;
        Ok(deployment)
    }

    async fn update_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
        command: UpdateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        let deployment = self
            .get_deployment_for_organisation(organisation_id, deployment_id)
            .await?;

        self.update_deployment(deployment.id, command).await
    }

    async fn delete_deployment(&self, deployment_id: DeploymentId) -> Result<(), CoreError> {
        self.deployment_repository.delete(deployment_id).await
    }

    async fn delete_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<(), CoreError> {
        let deployment = self
            .get_deployment_for_organisation(organisation_id, deployment_id)
            .await?;

        self.deployment_repository.delete(deployment.id).await
    }
}
