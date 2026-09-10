use crate::domain::entities::action_event::ActionEvent;
use crate::domain::entities::identity_instance::{DesiredIdentityInstance, IdentityInstanceRef};
use crate::domain::entities::outcome::DeploymentOutcomeReport;
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

/// Converges the cluster's `IdentityInstance` custom resources toward a desired state.
///
/// Implementations MUST be idempotent: calling `apply` repeatedly with an equal
/// `DesiredIdentityInstance` must converge to the same resource (a server-side apply,
/// never a blind create), and calling `delete` for an already-absent resource must
/// succeed rather than error. This is required by the at-least-once delivery semantics
/// of the upstream message bus (the same event can be redelivered).
pub trait IdentityInstancePort: Send + Sync {
    fn apply<'a>(
        &'a self,
        desired: &'a DesiredIdentityInstance,
    ) -> BoxFuture<'a, Result<(), GenesisError>>;

    fn delete<'a>(
        &'a self,
        reference: &'a IdentityInstanceRef,
    ) -> BoxFuture<'a, Result<(), GenesisError>>;
}

/// Drives the message-bus consumer loop.
pub trait EventConsumer: Send + Sync {
    /// Start consuming messages, dispatching each one to the registered handlers.
    fn run(&self) -> impl Future<Output = Result<(), GenesisError>> + Send;
}

/// Sends an outcome back towards the control plane.
///
/// A port rather than a direct call so the delete handler stays testable
/// without a broker, and so the transport can change without touching the
/// handler that decides *what* to report.
pub trait OutcomePublisher: Send + Sync {
    fn publish<'a>(
        &'a self,
        report: DeploymentOutcomeReport,
    ) -> BoxFuture<'a, Result<(), GenesisError>>;
}
