use crate::{
    AetherService, CoreError,
    action::{
        ActionBatch,
        commands::{FetchActionsCommand, RecordActionCommand},
        ports::ActionService,
    },
};

impl ActionService for AetherService {
    async fn get_action(
        &self,
        deployment_id: crate::domain::deployments::DeploymentId,
        action_id: crate::domain::action::ActionId,
    ) -> Result<Option<crate::action::Action>, CoreError> {
        self.action_service.get_action(deployment_id, action_id).await
    }

    async fn fetch_actions(&self, command: FetchActionsCommand) -> Result<ActionBatch, CoreError> {
        self.action_service.fetch_actions(command).await
    }

    async fn record_action(
        &self,
        command: RecordActionCommand,
    ) -> Result<crate::action::Action, CoreError> {
        self.action_service.record_action(command).await
    }
}
