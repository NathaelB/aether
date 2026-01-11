use crate::{
    AetherService, CoreError,
    deployments::{
        Deployment, DeploymentId,
        commands::{CreateDeploymentCommand, UpdateDeploymentCommand},
        ports::DeploymentService,
    },
    organisation::OrganisationId,
};

impl DeploymentService for AetherService {
    async fn create_deployment(
        &self,
        command: CreateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        self.deployment_service.create_deployment(command).await
    }

    async fn delete_deployment(&self, deployment_id: DeploymentId) -> Result<(), CoreError> {
        self.deployment_service
            .delete_deployment(deployment_id)
            .await
    }

    async fn delete_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<(), CoreError> {
        self.deployment_service
            .delete_deployment_for_organisation(organisation_id, deployment_id)
            .await
    }

    async fn get_deployment(
        &self,
        deployment_id: DeploymentId,
    ) -> Result<Option<Deployment>, CoreError> {
        self.deployment_service.get_deployment(deployment_id).await
    }

    async fn get_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<Deployment, CoreError> {
        self.deployment_service
            .get_deployment_for_organisation(organisation_id, deployment_id)
            .await
    }

    async fn list_deployments_by_organisation(
        &self,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Deployment>, CoreError> {
        self.deployment_service
            .list_deployments_by_organisation(organisation_id)
            .await
    }

    async fn update_deployment(
        &self,
        deployment_id: DeploymentId,
        command: UpdateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        self.deployment_service
            .update_deployment(deployment_id, command)
            .await
    }

    async fn update_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
        command: UpdateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        self.deployment_service
            .update_deployment_for_organisation(organisation_id, deployment_id, command)
            .await
    }
}
