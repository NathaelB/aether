use std::future::Future;

use aether_auth::Identity;
use chrono::{DateTime, Utc};

use crate::CoreError;
use crate::action::commands::{
    AckActionsCommand, ClaimActionsCommand, FetchActionsCommand, RecordActionCommand,
};
use crate::action::{Action, ActionBatch, ActionCursor, ActionFailureReason, ActionId};
use crate::deployments::DeploymentId;

#[cfg_attr(test, mockall::automock)]
pub trait ActionRepository: Send + Sync {
    fn append(&self, action: Action) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn get_by_id(
        &self,
        deployment_id: DeploymentId,
        action_id: ActionId,
    ) -> impl Future<Output = Result<Option<Action>, CoreError>> + Send;

    fn list(
        &self,
        deployment_id: DeploymentId,
        cursor: Option<ActionCursor>,
        limit: usize,
    ) -> impl Future<Output = Result<ActionBatch, CoreError>> + Send;

    /// Claims up to `max` actions for a deployment, leasing them until
    /// `lease_until`.
    ///
    /// An action whose lease expired before `now` is claimed again. A lease is
    /// a promise to publish, and the holder that let it lapse is gone: leaving
    /// the action `Leased` would strand the deployment on a claim nobody is
    /// still honouring.
    fn claim_pending(
        &self,
        deployment_id: DeploymentId,
        max: usize,
        now: DateTime<Utc>,
        lease_until: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<Action>, CoreError>> + Send;

    /// Transitions a currently-leased action to `Published`.
    ///
    /// Returns `true` if the action was leased and got transitioned, `false`
    /// if it did not exist or was not currently leased (no-op).
    fn ack_published(
        &self,
        deployment_id: DeploymentId,
        action_id: ActionId,
        at: DateTime<Utc>,
    ) -> impl Future<Output = Result<bool, CoreError>> + Send;

    /// Transitions a currently-leased action to `Failed`.
    ///
    /// Returns `true` if the action was leased and got transitioned, `false`
    /// if it did not exist or was not currently leased (no-op).
    fn ack_failed(
        &self,
        deployment_id: DeploymentId,
        action_id: ActionId,
        reason: ActionFailureReason,
        at: DateTime<Utc>,
    ) -> impl Future<Output = Result<bool, CoreError>> + Send;
}

#[cfg_attr(test, mockall::automock)]
pub trait ActionService: Send + Sync {
    fn record_action(
        &self,
        command: RecordActionCommand,
    ) -> impl Future<Output = Result<Action, CoreError>> + Send;

    fn get_action(
        &self,
        deployment_id: DeploymentId,
        action_id: ActionId,
    ) -> impl Future<Output = Result<Option<Action>, CoreError>> + Send;

    fn fetch_actions(
        &self,
        command: FetchActionsCommand,
        identity: Identity,
    ) -> impl Future<Output = Result<ActionBatch, CoreError>> + Send;

    fn claim_actions(
        &self,
        identity: Identity,
        command: ClaimActionsCommand,
    ) -> impl Future<Output = Result<Vec<Action>, CoreError>> + Send;

    /// Acknowledges the outcome of previously-claimed actions, moving them to
    /// a terminal state (`Published` or `Failed`). Returns the number of
    /// actions actually acknowledged (existing and currently leased).
    fn ack_actions(
        &self,
        identity: Identity,
        command: AckActionsCommand,
    ) -> impl Future<Output = Result<usize, CoreError>> + Send;
}
