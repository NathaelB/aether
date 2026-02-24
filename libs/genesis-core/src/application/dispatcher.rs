use crate::domain::entities::action_event::ActionEvent;
use crate::domain::error::GenesisError;
use crate::domain::ports::EventHandler;
use std::sync::Arc;
use tracing::{debug, warn};

/// Routes an incoming [`ActionEvent`] to the first handler whose routing key matches.
///
/// Matching rules (in order):
/// 1. Exact match on `routing_key` (e.g. `"deployment.create"`)
/// 2. Wildcard `"*"` matches any event
pub struct EventDispatcher {
    handlers: Vec<Arc<dyn EventHandler>>,
}

impl EventDispatcher {
    pub fn new(handlers: Vec<Arc<dyn EventHandler>>) -> Self {
        Self { handlers }
    }

    pub async fn dispatch(&self, event: ActionEvent) -> Result<(), GenesisError> {
        let matched = self.handlers.iter().find(|h| {
            let key = h.routing_key();
            key == "*" || key == event.routing_key
        });

        match matched {
            Some(handler) => {
                debug!(
                    routing_key = %event.routing_key,
                    action_id = %event.action_id,
                    "dispatching event"
                );
                handler.handle(event).await
            }
            None => {
                warn!(routing_key = %event.routing_key, "no handler registered for routing key");
                Ok(())
            }
        }
    }
}
