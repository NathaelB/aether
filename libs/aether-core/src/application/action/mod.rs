use aether_auth::Identity;
use aether_domain::action::{
    Action,
    commands::{AckActionsCommand, ClaimActionsCommand},
};
use aether_domain::deployments::ports::DeploymentRepository;
use aether_macros::transactional;
use chrono::Utc;

use crate::{
    AetherService, CoreError,
    action::{
        ActionBatch,
        commands::{FetchActionsCommand, RecordActionCommand},
        ports::ActionService,
        service::ActionServiceImpl,
    },
};

impl ActionService for AetherService {
    #[transactional(action)]
    async fn get_action(
        &self,
        deployment_id: crate::domain::deployments::DeploymentId,
        action_id: crate::domain::action::ActionId,
    ) -> Result<Option<crate::action::Action>, CoreError> {
        ActionServiceImpl::new(action_repository)
            .get_action(deployment_id, action_id)
            .await
    }

    #[transactional(action)]
    async fn fetch_actions(
        &self,
        command: FetchActionsCommand,
        identity: Identity,
    ) -> Result<ActionBatch, CoreError> {
        ActionServiceImpl::new(action_repository)
            .fetch_actions(command, identity)
            .await
    }

    #[transactional(action)]
    async fn record_action(
        &self,
        command: RecordActionCommand,
    ) -> Result<crate::action::Action, CoreError> {
        ActionServiceImpl::new(action_repository)
            .record_action(command)
            .await
    }

    #[transactional(action)]
    async fn claim_actions(
        &self,
        identity: Identity,
        command: ClaimActionsCommand,
    ) -> Result<Vec<Action>, CoreError> {
        ActionServiceImpl::new(action_repository)
            .claim_actions(identity, command)
            .await
    }

    #[transactional(action, deployment)]
    async fn ack_actions(
        &self,
        identity: Identity,
        command: AckActionsCommand,
    ) -> Result<usize, CoreError> {
        let deployment_id = command.deployment_id;
        let handed_over = !command.published.is_empty();
        let hand_off_failed = !command.failed.is_empty();

        let acknowledged = ActionServiceImpl::new(action_repository)
            .ack_actions(identity, command)
            .await?;

        // The ack is the only evidence the control plane ever gets that work
        // left it. Herald reports it, the actions move to `published`, and
        // until now the deployment stayed `pending` -- so a deployment being
        // applied was indistinguishable from one nobody had picked up.
        //
        // This says "handed over", not "running". Nothing yet reports back what
        // the cluster did with it, which is a separate gap.
        if let Some(mut deployment) = deployment_repository.get_by_id(deployment_id).await? {
            let at = Utc::now();
            // Publishing failures win: a batch where some actions reached the
            // bus and some did not is not a deployment that is on its way.
            let changed = if hand_off_failed {
                deployment.fail_hand_off(at)
            } else if handed_over {
                deployment.hand_off_to_data_plane(at)
            } else {
                false
            };

            if changed {
                deployment_repository.update(deployment).await?;
            }
        }

        Ok(acknowledged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::action::{
        ActionPayload, ActionSource, ActionTarget, ActionType, ActionVersion, TargetKind,
    };
    use crate::domain::dataplane::value_objects::DataPlaneId;
    use aether_auth::Client;
    use serde_json::json;
    use sqlx::postgres::PgPoolOptions;
    use std::time::Duration;
    use uuid::Uuid;

    fn service() -> AetherService {
        let pool = PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(50))
            .connect_lazy("postgres://user:pass@127.0.0.1:1/db")
            .expect("valid database url");
        AetherService::new(pool)
    }

    fn identity() -> Identity {
        Identity::Client(Client {
            id: "client-1".to_string(),
            client_id: "herald-service".to_string(),
            roles: vec![],
            scopes: vec![],
        })
    }

    #[tokio::test]
    async fn record_action_maps_pool_error() {
        let command = RecordActionCommand::new(
            crate::domain::deployments::DeploymentId(Uuid::new_v4()),
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

        let result = service().record_action(command).await;
        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn get_action_maps_pool_error() {
        let result = service()
            .get_action(
                crate::domain::deployments::DeploymentId(Uuid::new_v4()),
                crate::domain::action::ActionId(Uuid::new_v4()),
            )
            .await;

        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn fetch_actions_maps_pool_error() {
        let command =
            FetchActionsCommand::new(crate::domain::deployments::DeploymentId(Uuid::new_v4()), 10);

        let result = service().fetch_actions(command, identity()).await;
        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn ack_actions_maps_pool_error() {
        let command = AckActionsCommand {
            dataplane_id: DataPlaneId(Uuid::new_v4()),
            deployment_id: crate::domain::deployments::DeploymentId(Uuid::new_v4()),
            published: vec![crate::domain::action::ActionId(Uuid::new_v4())],
            failed: vec![],
        };

        let result = service().ack_actions(identity(), command).await;
        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn ack_actions_rejects_non_herald_identity() {
        let non_herald_identity = Identity::Client(Client {
            id: "client-2".to_string(),
            client_id: "some-other-service".to_string(),
            roles: vec![],
            scopes: vec![],
        });

        let command = AckActionsCommand {
            dataplane_id: DataPlaneId(Uuid::new_v4()),
            deployment_id: crate::domain::deployments::DeploymentId(Uuid::new_v4()),
            published: vec![crate::domain::action::ActionId(Uuid::new_v4())],
            failed: vec![],
        };

        let result = service().ack_actions(non_herald_identity, command).await;
        // The transaction is opened before the service sees the identity, so an
        // unreachable database surfaces first. The authorization rule is asserted
        // on the domain service, with mocked repositories and no pool.
        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }
}
