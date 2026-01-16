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
        #[cfg(feature = "test-mocks")]
        if let Some(action_repository) = crate::test_mocks::action_repository() {
            let action_service = ActionServiceImpl::new(action_repository);

            return action_service.get_action(deployment_id, action_id).await;
        }

        let action_repository = PostgresActionRepository::from_pool(self.pool());
        let action_service = ActionServiceImpl::new(action_repository);

        action_service.get_action(deployment_id, action_id).await
    }

    async fn fetch_actions(&self, command: FetchActionsCommand) -> Result<ActionBatch, CoreError> {
        #[cfg(feature = "test-mocks")]
        if let Some(action_repository) = crate::test_mocks::action_repository() {
            let action_service = ActionServiceImpl::new(action_repository);

            return action_service.fetch_actions(command).await;
        }

        let action_repository = PostgresActionRepository::from_pool(self.pool());
        let action_service = ActionServiceImpl::new(action_repository);

        action_service.fetch_actions(command).await
    }

    async fn record_action(
        &self,
        command: RecordActionCommand,
    ) -> Result<crate::action::Action, CoreError> {
        #[cfg(feature = "test-mocks")]
        if let Some(action_repository) = crate::test_mocks::action_repository() {
            let action_service = ActionServiceImpl::new(action_repository);

            return action_service.record_action(command).await;
        }

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
