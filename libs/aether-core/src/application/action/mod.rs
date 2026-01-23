use crate::{
    AetherService, CoreError,
    action::{
        ActionBatch,
        commands::{FetchActionsCommand, RecordActionCommand},
        ports::ActionService,
        service::ActionServiceImpl,
    },
    infrastructure::action::PostgresActionRepository,
};

impl ActionService for AetherService {
    async fn get_action(
        &self,
        deployment_id: crate::domain::deployments::DeploymentId,
        action_id: crate::domain::action::ActionId,
    ) -> Result<Option<crate::action::Action>, CoreError> {
        let action_repository = PostgresActionRepository::from_pool(self.pool());
        let action_service = ActionServiceImpl::new(action_repository);

        action_service.get_action(deployment_id, action_id).await
    }

    async fn fetch_actions(&self, command: FetchActionsCommand) -> Result<ActionBatch, CoreError> {
        let action_repository = PostgresActionRepository::from_pool(self.pool());
        let action_service = ActionServiceImpl::new(action_repository);

        action_service.fetch_actions(command).await
    }

    async fn record_action(
        &self,
        command: RecordActionCommand,
    ) -> Result<crate::action::Action, CoreError> {
        let tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;
        let tx = tokio::sync::Mutex::new(Some(tx));

        let result = {
            let action_repository = PostgresActionRepository::from_tx(&tx);
            let action_service = ActionServiceImpl::new(action_repository);

            action_service.record_action(command).await
        };

        match result {
            Ok(action) => {
                super::take_transaction(&tx)
                    .await?
                    .commit()
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                Ok(action)
            }
            Err(err) => {
                super::take_transaction(&tx)
                    .await?
                    .rollback()
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::action::{
        ActionPayload, ActionSource, ActionTarget, ActionType, ActionVersion, TargetKind,
    };
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

    #[tokio::test]
    async fn record_action_maps_pool_error() {
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

        let result = service().fetch_actions(command).await;
        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }
}
