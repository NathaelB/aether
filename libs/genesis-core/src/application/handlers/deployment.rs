use crate::domain::entities::action_event::ActionEvent;
use crate::domain::error::GenesisError;
use crate::domain::ports::{BoxFuture, EventHandler};
use tracing::info;

/// Handles events with routing key `deployment.<kind>`.
pub struct DeploymentEventHandler {
    routing_key: String,
}

impl DeploymentEventHandler {
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            routing_key: format!("deployment.{}", kind.into()),
        }
    }
}

impl EventHandler for DeploymentEventHandler {
    fn routing_key(&self) -> &str {
        &self.routing_key
    }

    fn handle<'a>(&'a self, event: ActionEvent) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async move {
            info!(
                action_id = %event.action_id,
                routing_key = %event.routing_key,
                payload = %event.payload,
                "handling deployment event"
            );

            // TODO: implement business logic (e.g. trigger Kubernetes reconciliation)

            Ok(())
        })
    }
}
