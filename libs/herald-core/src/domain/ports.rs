use crate::domain::entities::action::{AckFailure, AckOutcome, Action, ActionEvent, ActionId};
use crate::domain::entities::dataplane::DataPlaneId;
use crate::domain::entities::deployment::{Deployment, DeploymentId};
use crate::domain::error::HeraldError;
use std::future::Future;

pub trait HeraldService: Send + Sync {
    fn sync_all_deployments(&self) -> impl Future<Output = Result<(), HeraldError>> + Send;
    fn process_deployment(
        &self,
        deployment_id: &DeploymentId,
    ) -> impl Future<Output = Result<(), HeraldError>> + Send;
}

#[cfg_attr(test, mockall::automock)]
pub trait ControlPlaneRepository: Send + Sync {
    fn list_deployments(
        &self,
        dp_id: &DataPlaneId,
    ) -> impl Future<Output = Result<Vec<Deployment>, HeraldError>> + Send;

    fn claim_actions(
        &self,
        dp_id: &DataPlaneId,
        deployment_id: &DeploymentId,
    ) -> impl Future<Output = Result<Vec<Action>, HeraldError>> + Send;

    /// Acknowledges the outcome of previously-claimed actions. At-least-once
    /// delivery relies on this happening *after* a successful publish: if the
    /// ack is lost, the lease expires and the action is republished, which is
    /// why consumers deduplicate on `action_id`.
    fn ack_actions(
        &self,
        dp_id: &DataPlaneId,
        deployment_id: &DeploymentId,
        published: Vec<ActionId>,
        failed: Vec<AckFailure>,
    ) -> impl Future<Output = Result<AckOutcome, HeraldError>> + Send;
}

#[cfg_attr(test, mockall::automock)]
pub trait MessageBusRepository: Send + Sync {
    fn publish(&self, event: ActionEvent) -> impl Future<Output = Result<(), HeraldError>> + Send;
}
