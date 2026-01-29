use crate::domain::HeraldError;
use crate::domain::action::Action;
use crate::domain::dataplane::DataPlaneId;
use crate::domain::deployment::{Deployment, DeploymentId};
use async_trait::async_trait;

#[async_trait]
pub trait ControlPlane: Send + Sync {
    /// Récupère la liste des déploiements pour un DataPlane donné
    async fn list_deployments(&self, dp_id: &DataPlaneId) -> Result<Vec<Deployment>, HeraldError>;

    /// Claim les actions pour un DataPlane donné
    async fn claim_actions(
        &self,
        dp_id: &DataPlaneId,
        deployment_id: &DeploymentId,
    ) -> Result<Vec<Action>, HeraldError>;
}
