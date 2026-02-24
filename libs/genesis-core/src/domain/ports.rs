use crate::domain::entities::action_event::ActionEvent;
use crate::domain::error::GenesisError;
use std::future::Future;
use std::pin::Pin;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Implemented by application-layer handlers that react to a specific routing key.
pub trait EventHandler: Send + Sync {
    /// The routing key pattern this handler subscribes to (e.g. `"deployment.create"`).
    fn routing_key(&self) -> &str;

    /// Process an incoming event. Called by the consumer for each matching message.
    fn handle<'a>(&'a self, event: ActionEvent) -> BoxFuture<'a, Result<(), GenesisError>>;
}

/// Drives the message-bus consumer loop.
pub trait EventConsumer: Send + Sync {
    /// Start consuming messages, dispatching each one to the registered handlers.
    fn run(&self) -> impl Future<Output = Result<(), GenesisError>> + Send;
}
