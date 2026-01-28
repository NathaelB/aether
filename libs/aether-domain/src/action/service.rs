use aether_auth::Identity;
use chrono::Utc;
use tracing::info;
use uuid::Uuid;

use crate::CoreError;
use crate::action::ActionBatch;
use crate::action::{
    Action, ActionId, ActionMetadata, ActionStatus,
    commands::{FetchActionsCommand, RecordActionCommand},
    ports::{ActionRepository, ActionService},
};

#[derive(Debug)]
pub struct ActionServiceImpl<R>
where
    R: ActionRepository,
{
    action_repository: R,
}

impl<R> ActionServiceImpl<R>
where
    R: ActionRepository,
{
    pub fn new(repository: R) -> Self {
        Self {
            action_repository: repository,
        }
    }
}

impl<R> ActionService for ActionServiceImpl<R>
where
    R: ActionRepository,
{
    async fn record_action(&self, command: RecordActionCommand) -> Result<Action, CoreError> {
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

    async fn get_action(
        &self,
        deployment_id: crate::deployments::DeploymentId,
        action_id: ActionId,
    ) -> Result<Option<Action>, CoreError> {
        self.action_repository
            .get_by_id(deployment_id, action_id)
            .await
    }

    async fn fetch_actions(
        &self,
        command: FetchActionsCommand,
        identity: Identity,
    ) -> Result<ActionBatch, CoreError> {
        let client_id = identity.username();
        info!("the client: {} try to fetch actions", client_id);

        if client_id != "herald-service" {
            return Err(CoreError::PermissionDenied {
                reason: "you can't fetch actions".to_string(),
            });
        }

        self.action_repository
            .list(command.deployment_id, command.cursor, command.limit)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_auth::Client;
    use crate::action::{
        ActionBatch, ActionConstraints, ActionCursor, ActionPayload, ActionSource, ActionTarget,
        ActionType, ActionVersion, TargetKind, ports::MockActionRepository,
    };
    use crate::deployments::DeploymentId;
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
                    && action.payload
                        == ActionPayload {
                            data: json!({"id": "dep-1"}),
                        }
                    && action.metadata.source == ActionSource::System
                    && action.metadata.constraints
                        == ActionConstraints {
                            not_after: None,
                            priority: None,
                        }
            })
            .returning(|_| Box::pin(async { Ok(()) }));

        let service = ActionServiceImpl::new(mock_repo);
        let command = RecordActionCommand::new(
            ActionType("deployment.create".to_string()),
            ActionTarget {
                kind: TargetKind::Deployment,
                id: Uuid::new_v4(),
            },
            ActionPayload {
                data: json!({"id": "dep-1"}),
            },
            ActionVersion(1),
            ActionSource::System,
        );

        let result = service.record_action(command).await;
        assert!(result.is_ok());
        let action = result.unwrap();
        assert!(matches!(action.status, ActionStatus::Pending));
        assert_eq!(
            action.action_type,
            ActionType("deployment.create".to_string())
        );
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

        let service = ActionServiceImpl::new(mock_repo);
        let command =
            FetchActionsCommand::new(deployment_id, 25).with_cursor(ActionCursor::new("cursor-1"));
        let identity = Identity::Client(Client {
            id: "client-1".to_string(),
            client_id: "herald-service".to_string(),
            roles: vec![],
            scopes: vec![],
        });

        let result = service.fetch_actions(command, identity).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_batch);
    }

    #[tokio::test]
    async fn get_action_delegates_to_repository() {
        let mut mock_repo = MockActionRepository::new();
        let deployment_id = DeploymentId(Uuid::new_v4());
        let action_id = ActionId(Uuid::new_v4());
        let action = Action {
            id: action_id,
            action_type: ActionType("deployment.create".to_string()),
            target: ActionTarget {
                kind: TargetKind::Deployment,
                id: deployment_id.0,
            },
            payload: ActionPayload {
                data: json!({"id": "dep-1"}),
            },
            version: ActionVersion(1),
            status: ActionStatus::Pending,
            metadata: ActionMetadata {
                source: ActionSource::System,
                created_at: Utc::now(),
                constraints: ActionConstraints::default(),
            },
        };

        mock_repo
            .expect_get_by_id()
            .times(1)
            .withf(move |id, act_id| *id == deployment_id && *act_id == action_id)
            .returning(move |_, _| {
                let action = action.clone();
                Box::pin(async move { Ok(Some(action)) })
            });

        let service = ActionServiceImpl::new(mock_repo);
        let result = service.get_action(deployment_id, action_id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().unwrap().id, action_id);
    }
}
