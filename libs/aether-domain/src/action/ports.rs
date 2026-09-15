use std::future::Future;

use chrono::{DateTime, Utc};

use crate::CoreError;
use crate::action::commands::RecordActionCommand;
use crate::action::{Action, ActionBatch, ActionCursor, ActionFailureReason, ActionId, ActionType};
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

    /// When this deployment was last told to do a thing of this kind.
    ///
    /// `None` when it never was. Asked rather than derived from a listing: the
    /// one caller wants the most recent of one type, and paging a deployment's
    /// whole history to find it would grow with the deployment's age.
    fn last_of_type(
        &self,
        deployment_id: DeploymentId,
        action_type: &ActionType,
    ) -> impl Future<Output = Result<Option<DateTime<Utc>>, CoreError>> + Send;

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
/// What the API layer calls to record an action.
///
/// The three a data plane calls are not here. They take proof of which data
/// plane is speaking, which is read from a credential and cannot be built by a
/// handler -- so they are inherent methods on the services that can obtain it,
/// and the port keeps the one use case a request can express on its own.
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
}
