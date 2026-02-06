use std::future::Future;

use crate::{
    CoreError,
    dataplane::value_objects::DataPlaneId,
    deployments::{
        Deployment, DeploymentId,
        commands::{CreateDeploymentCommand, UpdateDeploymentCommand},
    },
    organisation::OrganisationId,
};

/// Service trait for deployment business logic
pub trait DeploymentService: Send + Sync {
    /// Creates a new deployment
    fn create_deployment(
        &self,
        command: CreateDeploymentCommand,
    ) -> impl Future<Output = Result<Deployment, CoreError>> + Send;

    /// Fetches a deployment by ID
    fn get_deployment(
        &self,
        deployment_id: DeploymentId,
    ) -> impl Future<Output = Result<Option<Deployment>, CoreError>> + Send;

    /// Fetches a deployment by ID scoped to an organisation
    fn get_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> impl Future<Output = Result<Deployment, CoreError>> + Send;

    /// Lists deployments for an organisation
    fn list_deployments_by_organisation(
        &self,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<Vec<Deployment>, CoreError>> + Send;

    /// Updates an existing deployment
    fn update_deployment(
        &self,
        deployment_id: DeploymentId,
        command: UpdateDeploymentCommand,
    ) -> impl Future<Output = Result<Deployment, CoreError>> + Send;

    /// Updates an existing deployment scoped to an organisation
    fn update_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
        command: UpdateDeploymentCommand,
    ) -> impl Future<Output = Result<Deployment, CoreError>> + Send;

    /// Deletes a deployment
    fn delete_deployment(
        &self,
        deployment_id: DeploymentId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    /// Deletes a deployment scoped to an organisation
    fn delete_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;
}

/// Repository trait for managing Deployment entities.
/// This trait defines the necessary methods for inserting, retrieving,
/// listing, updating, and deleting Deployment records in a data store.
/// Implementors of this trait must ensure thread safety by being Send and Sync.
#[cfg_attr(test, mockall::automock)]
pub trait DeploymentRepository: Send + Sync {
    fn insert(&self, deployment: Deployment) -> impl Future<Output = Result<(), CoreError>> + Send;
    fn get_by_id(
        &self,
        deployment_id: DeploymentId,
    ) -> impl Future<Output = Result<Option<Deployment>, CoreError>> + Send;

    fn list_by_organisation(
        &self,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<Vec<Deployment>, CoreError>> + Send;

    fn update(&self, deployment: Deployment) -> impl Future<Output = Result<(), CoreError>> + Send;
    fn delete(
        &self,
        deployment_id: DeploymentId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;
    fn list_by_dataplane(
        &self,
        dataplane_id: &DataPlaneId,
    ) -> impl Future<Output = Result<Vec<Deployment>, CoreError>> + Send;
}
