use crate::domain::action::{Action, ActionEvent};
use crate::domain::dataplane::DataPlaneId;
use crate::domain::deployment::{Deployment, DeploymentId};
use crate::domain::error::HeraldError;
use async_trait::async_trait;

#[async_trait]
pub trait HeraldService: Send + Sync {
    async fn sync_all_deployments(&self) -> Result<(), HeraldError>;
    async fn process_deployment(&self, deployment_id: &DeploymentId) -> Result<(), HeraldError>;
}

#[async_trait]
pub trait ControlPlaneRepository: Send + Sync {
    async fn list_deployments(&self, dp_id: &DataPlaneId) -> Result<Vec<Deployment>, HeraldError>;

    async fn claim_actions(
        &self,
        dp_id: &DataPlaneId,
        deployment_id: &DeploymentId,
    ) -> Result<Vec<Action>, HeraldError>;
}

#[async_trait]
pub trait MessageBusRepository: Send + Sync {
    async fn publish(&self, event: ActionEvent) -> Result<(), HeraldError>;
}
