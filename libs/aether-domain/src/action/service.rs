use aether_auth::Identity;
use chrono::{Duration, Utc};
use tracing::info;
use uuid::Uuid;

use crate::CoreError;
use crate::action::ActionBatch;
use crate::action::commands::{AckActionsCommand, ClaimActionsCommand};
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
            deployment_id: command.deployment_id,
            dataplane_id: command.dataplane_id,
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
            leased_until: None,
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

    async fn claim_actions(
        &self,
        identity: Identity,
        command: ClaimActionsCommand,
    ) -> Result<Vec<Action>, CoreError> {
        let client_id = identity.username();

        info!("the client: {} try to claim actions", client_id);

        if !client_id.contains("herald-service") {
            return Err(CoreError::PermissionDenied {
                reason: "only herald can claim actions".to_string(),
            });
        }

        let lease_until = Utc::now() + Duration::seconds(command.lease_seconds);

        let actions = self
            .action_repository
            .claim_pending(command.deployment_id, command.max, lease_until)
            .await?;

        Ok(actions)
    }

    async fn ack_actions(
        &self,
        identity: Identity,
        command: AckActionsCommand,
    ) -> Result<usize, CoreError> {
        let client_id = identity.username();

        info!("the client: {} try to ack actions", client_id);

        if !client_id.contains("herald-service") {
            return Err(CoreError::PermissionDenied {
                reason: "only herald can ack actions".to_string(),
            });
        }

        let at = Utc::now();
        let mut acknowledged = 0usize;

        for action_id in command.published {
            if self
                .action_repository
                .ack_published(command.deployment_id, action_id, at)
                .await?
            {
                acknowledged += 1;
            }
        }

        for failure in command.failed {
            if self
                .action_repository
                .ack_failed(command.deployment_id, failure.action_id, failure.reason, at)
                .await?
            {
                acknowledged += 1;
            }
        }

        Ok(acknowledged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::commands::AckFailure;
    use crate::action::{
        ActionBatch, ActionConstraints, ActionCursor, ActionFailureReason, ActionPayload,
        ActionSource, ActionTarget, ActionType, ActionVersion, TargetKind,
        ports::MockActionRepository,
    };
    use crate::dataplane::value_objects::DataPlaneId;
    use crate::deployments::DeploymentId;
    use aether_auth::Client;
    use serde_json::json;

    fn herald_identity() -> Identity {
        Identity::Client(Client {
            id: "client-1".to_string(),
            client_id: "herald-service".to_string(),
            roles: vec![],
            scopes: vec![],
        })
    }

    fn non_herald_identity() -> Identity {
        Identity::Client(Client {
            id: "client-2".to_string(),
            client_id: "some-other-service".to_string(),
            roles: vec![],
            scopes: vec![],
        })
    }

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
            DeploymentId(Uuid::new_v4()),
            DataPlaneId(Uuid::new_v4()),
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
            deployment_id,
            dataplane_id: DataPlaneId(Uuid::new_v4()),
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
            leased_until: None,
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

    #[tokio::test]
    async fn ack_actions_transitions_leased_action_to_published() {
        let mut mock_repo = MockActionRepository::new();
        let deployment_id = DeploymentId(Uuid::new_v4());
        let action_id = ActionId(Uuid::new_v4());

        mock_repo
            .expect_ack_published()
            .times(1)
            .withf(move |dep_id, act_id, _at| *dep_id == deployment_id && *act_id == action_id)
            .returning(|_, _, _| Box::pin(async { Ok(true) }));

        let service = ActionServiceImpl::new(mock_repo);
        let command = AckActionsCommand {
            dataplane_id: DataPlaneId(Uuid::new_v4()),
            deployment_id,
            published: vec![action_id],
            failed: vec![],
        };

        let result = service.ack_actions(herald_identity(), command).await;
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn ack_actions_transitions_leased_action_to_failed() {
        let mut mock_repo = MockActionRepository::new();
        let deployment_id = DeploymentId(Uuid::new_v4());
        let action_id = ActionId(Uuid::new_v4());

        mock_repo
            .expect_ack_failed()
            .times(1)
            .withf(move |dep_id, act_id, reason, _at| {
                *dep_id == deployment_id
                    && *act_id == action_id
                    && *reason == ActionFailureReason::Timeout
            })
            .returning(|_, _, _, _| Box::pin(async { Ok(true) }));

        let service = ActionServiceImpl::new(mock_repo);
        let command = AckActionsCommand {
            dataplane_id: DataPlaneId(Uuid::new_v4()),
            deployment_id,
            published: vec![],
            failed: vec![AckFailure {
                action_id,
                reason: ActionFailureReason::Timeout,
            }],
        };

        let result = service.ack_actions(herald_identity(), command).await;
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn ack_actions_unleased_or_unknown_id_is_a_noop() {
        let mut mock_repo = MockActionRepository::new();
        let deployment_id = DeploymentId(Uuid::new_v4());
        let action_id = ActionId(Uuid::new_v4());

        mock_repo
            .expect_ack_published()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(false) }));

        let service = ActionServiceImpl::new(mock_repo);
        let command = AckActionsCommand {
            dataplane_id: DataPlaneId(Uuid::new_v4()),
            deployment_id,
            published: vec![action_id],
            failed: vec![],
        };

        let result = service.ack_actions(herald_identity(), command).await;
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn ack_actions_same_id_twice_counts_once() {
        let mut mock_repo = MockActionRepository::new();
        let deployment_id = DeploymentId(Uuid::new_v4());
        let action_id = ActionId(Uuid::new_v4());

        // First ack transitions Leased -> Published (counted); the retry
        // finds it already Published (not leased), so it is a no-op.
        let mut call_count = 0;
        mock_repo
            .expect_ack_published()
            .times(2)
            .returning(move |_, _, _| {
                call_count += 1;
                let first_call = call_count == 1;
                Box::pin(async move { Ok(first_call) })
            });

        let service = ActionServiceImpl::new(mock_repo);
        let command = AckActionsCommand {
            dataplane_id: DataPlaneId(Uuid::new_v4()),
            deployment_id,
            published: vec![action_id, action_id],
            failed: vec![],
        };

        let result = service.ack_actions(herald_identity(), command).await;
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn ack_actions_rejects_non_herald_identity() {
        let mock_repo = MockActionRepository::new();
        let deployment_id = DeploymentId(Uuid::new_v4());

        let service = ActionServiceImpl::new(mock_repo);
        let command = AckActionsCommand {
            dataplane_id: DataPlaneId(Uuid::new_v4()),
            deployment_id,
            published: vec![ActionId(Uuid::new_v4())],
            failed: vec![],
        };

        let result = service.ack_actions(non_herald_identity(), command).await;
        assert!(matches!(result, Err(CoreError::PermissionDenied { .. })));
    }
}
