use std::future::Future;

use chrono::{DateTime, Duration, Utc};

use crate::{
    CoreError,
    dataplane::value_objects::DataPlaneId,
    deployments::{
        Deployment, DeploymentId, DeploymentKind,
        commands::{CreateDeploymentCommand, UpdateDeploymentCommand},
    },
    organisation::OrganisationId,
    version::Version,
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

    /// Removes deployments whose tear-down was confirmed longer ago than the
    /// retention window. Returns how many.
    fn purge_deleted_deployments(
        &self,
        retention: Duration,
    ) -> impl Future<Output = Result<u64, CoreError>> + Send;

    /// Deletes a deployment, and returns what was deleted.
    ///
    /// Returning it is not a convenience: deletion is a soft delete in the
    /// control plane, and the data plane only learns about it through a
    /// `deployment.delete` action carrying the namespace and the data plane
    /// the resources actually live on. The caller cannot record that action
    /// without the deployment, and a deletion that records nothing leaves the
    /// row in `deleting` for ever with the Kubernetes resources still running.
    fn delete_deployment(
        &self,
        deployment_id: DeploymentId,
    ) -> impl Future<Output = Result<Deployment, CoreError>> + Send;

    /// Deletes a deployment scoped to an organisation, and returns what was
    /// deleted. See [`DeploymentService::delete_deployment`].
    fn delete_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> impl Future<Output = Result<Deployment, CoreError>> + Send;
}

/// Repository trait for managing Deployment entities.
/// This trait defines the necessary methods for inserting, retrieving,
/// listing, updating, and deleting Deployment records in a data store.
/// Implementors of this trait must ensure thread safety by being Send and Sync.
#[cfg_attr(test, mockall::automock)]
pub trait DeploymentRepository: Send + Sync {
    /// Removes deployments whose tear-down was confirmed before `before`.
    ///
    /// Only `deleted`, never `deleting`: a deployment still waiting for its
    /// tear-down to be confirmed is one that needs attention, and dropping it
    /// would erase the evidence that something is stuck.
    ///
    /// Returns how many were removed. The `actions` rows go with them, which
    /// is why this waits rather than running at confirmation time -- the
    /// history is worth keeping for a while.
    fn purge_deleted(
        &self,
        before: DateTime<Utc>,
    ) -> impl Future<Output = Result<u64, CoreError>> + Send;
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

    /// How many live deployments run each version of a product.
    ///
    /// Counted in SQL rather than by listing and grouping: this crosses every
    /// organisation, and loading them all to count them would be the one query
    /// that grows with the whole customer base.
    ///
    /// A deployment being torn down is not counted. It stops being a reason to
    /// keep a version alive the moment its removal is asked for.
    fn count_by_version(
        &self,
        kind: &DeploymentKind,
    ) -> impl Future<Output = Result<Vec<(Version, u64)>, CoreError>> + Send;
}
