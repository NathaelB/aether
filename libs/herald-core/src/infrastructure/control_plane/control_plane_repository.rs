use crate::domain::{
    entities::{
        action::Action,
        dataplane::DataPlaneId,
        deployment::{Deployment, DeploymentId},
    },
    error::HeraldError,
    ports::ControlPlaneRepository,
};

/// HTTP client implementation for communicating with the control plane API
pub struct HttpControlPlaneRepository {
    // TODO: Add reqwest::Client field
    // TODO: Add base_url field
}

impl HttpControlPlaneRepository {
    /// Creates a new instance of the HTTP control plane repository
    pub fn new(base_url: impl Into<String>) -> Self {
        let base_url = base_url.into();
        todo!("Initialize the HTTP client and store the {base_url}")
    }
}

impl ControlPlaneRepository for HttpControlPlaneRepository {
    async fn list_deployments(
        &self,
        dataplane_id: &DataPlaneId,
    ) -> Result<Vec<Deployment>, HeraldError> {
        todo!("Implement GET /dataplanes/{dataplane_id}/deployments")
    }

    async fn claim_actions(
        &self,
        dataplane_id: &DataPlaneId,
        deployment_id: &DeploymentId,
    ) -> Result<Vec<Action>, HeraldError> {
        todo!("Implement POST /dataplanes/{dataplane_id}/deployments/{deployment_id}/actions:claim")
    }
}
