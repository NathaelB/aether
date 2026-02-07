use std::future::Future;

use aether_auth::Identity;
use chrono::{DateTime, Utc};

use crate::CoreError;
use crate::action::commands::{ClaimActionsCommand, FetchActionsCommand, RecordActionCommand};
use crate::action::{Action, ActionBatch, ActionCursor, ActionId};
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

    fn claim_pending(
        &self,
        deployment_id: DeploymentId,
        max: usize,
        lease_until: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<Action>, CoreError>> + Send;
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
}
