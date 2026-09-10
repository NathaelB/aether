use crate::domain::entities::action::{AckFailure, AckOutcome, Action, ActionEvent, ActionId};
use crate::domain::entities::dataplane::DataPlaneId;
use crate::domain::entities::deployment::{Deployment, DeploymentId};
use crate::domain::entities::outcome::DeploymentOutcomeReport;
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

    /// Reports that this data plane is alive.
    ///
    /// Sent once per sync cycle. Without it the control plane cannot tell a
    /// data plane that is idle from one that is gone, and keeps placing new
    /// deployments on a cluster that will never claim them.
    fn send_heartbeat(
        &self,
        dp_id: &DataPlaneId,
    ) -> impl Future<Output = Result<(), HeraldError>> + Send;

    /// Carries an outcome another data plane component observed.
    ///
    /// Herald is a courier here, not an author: it did not see the thing being
    /// reported, it just holds the credentials to say it.
    fn report_outcome(
        &self,
        dp_id: &DataPlaneId,
        report: &DeploymentOutcomeReport,
    ) -> impl Future<Output = Result<(), HeraldError>> + Send;
}

/// The outcomes waiting to be carried to the control plane.
#[cfg_attr(test, mockall::automock)]
pub trait OutcomeInboxRepository: Send + Sync {
    /// Takes everything queued, up to `limit`.
    ///
    /// Drained on the sync cycle rather than consumed continuously: outcomes
    /// are rare, and a second long-lived consumer would be a second thing that
    /// can silently stop. `limit` keeps one cycle bounded when a backlog has
    /// built up.
    fn drain(
        &self,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<DeploymentOutcomeReport>, HeraldError>> + Send;
}

#[cfg_attr(test, mockall::automock)]
pub trait MessageBusRepository: Send + Sync {
    fn publish(&self, event: ActionEvent) -> impl Future<Output = Result<(), HeraldError>> + Send;
}
