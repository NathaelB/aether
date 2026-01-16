use std::future::Future;

use crate::domain::CoreError;
use crate::domain::action::commands::{FetchActionsCommand, RecordActionCommand};
use crate::domain::action::{Action, ActionBatch, ActionCursor, ActionId};
use crate::domain::deployments::DeploymentId;

#[cfg_attr(any(test, feature = "test-mocks"), mockall::automock)]
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
}

#[cfg(any(test, feature = "test-mocks"))]
impl ActionRepository for std::sync::Arc<MockActionRepository> {
    fn append(&self, action: Action) -> impl Future<Output = Result<(), CoreError>> + Send {
        (**self).append(action)
    }

    fn get_by_id(
        &self,
        deployment_id: DeploymentId,
        action_id: ActionId,
    ) -> impl Future<Output = Result<Option<Action>, CoreError>> + Send {
        (**self).get_by_id(deployment_id, action_id)
    }

    fn list(
        &self,
        deployment_id: DeploymentId,
        cursor: Option<ActionCursor>,
        limit: usize,
    ) -> impl Future<Output = Result<ActionBatch, CoreError>> + Send {
        (**self).list(deployment_id, cursor, limit)
    }
}

#[cfg_attr(any(test, feature = "test-mocks"), mockall::automock)]
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
    ) -> impl Future<Output = Result<ActionBatch, CoreError>> + Send;
}
