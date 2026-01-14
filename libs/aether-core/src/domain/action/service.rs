use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use crate::domain::action::{
    Action, ActionId, ActionMetadata, ActionStatus,
    commands::{FetchActionsCommand, RecordActionCommand},
    ports::{ActionRepository, ActionService},
};
use crate::domain::CoreError;

#[derive(Clone, Debug)]
pub struct ActionServiceImpl<R>
where
    R: ActionRepository,
{
    action_repository: Arc<R>,
}

impl<R> ActionServiceImpl<R>
where
    R: ActionRepository,
{
    pub fn new(repository: Arc<R>) -> Self {
        Self {
            action_repository: repository,
        }
    }
}

impl<R> ActionService for ActionServiceImpl<R>
where
    R: ActionRepository,
{
    async fn record_action(
        &self,
        command: RecordActionCommand,
    ) -> Result<Action, CoreError> {
        let action = Action {
            id: ActionId(Uuid::new_v4()),
            action_type: command.action_type,
            target: command.target,
            payload: command.payload,
            version: command.version,
            status: ActionStatus::Pending,
            metadata: ActionMetadata {
                source: command.source,
                created_at: Utc::now(),
                constraints: command.constraints,
            },
        };

        self.action_repository.append(action.clone()).await?;

        Ok(action)
    }

    async fn fetch_actions(
        &self,
        command: FetchActionsCommand,
    ) -> Result<crate::domain::action::ActionBatch, CoreError> {
        self.action_repository
            .list(command.deployment_id, command.cursor, command.limit)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::action::{
        ActionBatch, ActionConstraints, ActionCursor, ActionPayload, ActionSource, ActionTarget,
        ActionType, ActionVersion, TargetKind,
        ports::MockActionRepository,
    };
    use crate::domain::deployments::DeploymentId;
    use serde_json::json;

    #[tokio::test]
    async fn record_action_persists_action() {
        let mut mock_repo = MockActionRepository::new();

        mock_repo
            .expect_append()
            .times(1)
            .withf(|action| {
                action.action_type == ActionType("deployment.create".to_string())
                    && matches!(action.status, ActionStatus::Pending)
                    && action.payload == ActionPayload { data: json!({"id": "dep-1"}) }
                    && action.metadata.source == ActionSource::System
                    && action.metadata.constraints == ActionConstraints { not_after: None, priority: None }
            })
            .returning(|_| Box::pin(async { Ok(()) }));

        let service = ActionServiceImpl::new(Arc::new(mock_repo));
        let command = RecordActionCommand::new(
            ActionType("deployment.create".to_string()),
            ActionTarget {
                kind: TargetKind::Deployment,
                id: Uuid::new_v4(),
            },
            ActionPayload { data: json!({"id": "dep-1"}) },
            ActionVersion(1),
            ActionSource::System,
        );

        let result = service.record_action(command).await;
        assert!(result.is_ok());
        let action = result.unwrap();
        assert!(matches!(action.status, ActionStatus::Pending));
        assert_eq!(action.action_type, ActionType("deployment.create".to_string()));
    }

    #[tokio::test]
    async fn fetch_actions_returns_batch() {
        let mut mock_repo = MockActionRepository::new();
        let deployment_id = DeploymentId(Uuid::new_v4());
        let expected_batch = ActionBatch {
            actions: vec![],
            next_cursor: Some(ActionCursor::new("cursor-1")),
        };
        let expected_batch_clone = expected_batch.clone();

        mock_repo
            .expect_list()
            .times(1)
            .withf(move |id, cursor, limit| {
                *id == deployment_id
                    && *limit == 25
                    && *cursor == Some(ActionCursor::new("cursor-1"))
            })
            .returning(move |_, _, _| {
                let batch = expected_batch_clone.clone();
                Box::pin(async move { Ok(batch) })
            });

        let service = ActionServiceImpl::new(Arc::new(mock_repo));
        let command =
            FetchActionsCommand::new(deployment_id, 25).with_cursor(ActionCursor::new("cursor-1"));

        let result = service.fetch_actions(command).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_batch);
    }
}
