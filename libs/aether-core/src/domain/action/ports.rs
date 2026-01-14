use std::future::Future;

use crate::domain::deployments::DeploymentId;
use crate::domain::CoreError;
use crate::domain::action::{Action, ActionBatch, ActionCursor};
use crate::domain::action::commands::{FetchActionsCommand, RecordActionCommand};

#[cfg_attr(test, mockall::automock)]
pub trait ActionRepository: Send + Sync {
    fn append(&self, action: Action) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn list(
        &self,
        deployment_id: DeploymentId,
        cursor: Option<ActionCursor>,
        limit: usize,
    ) -> impl Future<Output = Result<ActionBatch, CoreError>> + Send;
}

#[cfg_attr(test, mockall::automock)]
pub trait ActionService: Send + Sync {
    fn record_action(
        &self,
        command: RecordActionCommand,
    ) -> impl Future<Output = Result<Action, CoreError>> + Send;

    fn fetch_actions(
        &self,
        command: FetchActionsCommand,
    ) -> impl Future<Output = Result<ActionBatch, CoreError>> + Send;
}
