use crate::domain::entities::action_event::ActionEvent;
use crate::domain::entities::identity_instance::{
    DesiredIdentityInstance, DesiredUpgrade, IdentityInstanceRef, InFlightUpgrade, UpgradeRef,
};
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

    /// Replaces the ranges allowed to reach an instance.
    ///
    /// `None` is open. A merge patch on that one field rather than a
    /// re-apply of the whole spec: the action that carries an allow list
    /// carries nothing about the version, the database or the resources, and
    /// re-applying a spec rebuilt from defaults would quietly resize the
    /// instance on the way past.
    fn set_allowed_cidrs<'a>(
        &'a self,
        reference: &'a IdentityInstanceRef,
        ranges: Option<Vec<String>>,
    ) -> BoxFuture<'a, Result<(), GenesisError>>;

    /// Asks for one archive of this instance, now.
    ///
    /// `name` is the caller's, not generated here. Delivery is at-least-once
    /// by design, and a name invented per delivery would archive the same
    /// database twice for one request -- at the cost of reading the whole of
    /// it, twice.
    fn take_archive<'a>(
        &'a self,
        reference: &'a IdentityInstanceRef,
        name: &'a str,
    ) -> BoxFuture<'a, Result<(), GenesisError>>;

    /// Whether the instance is fully up: the database, the app and its
    /// migration combined into the one signal a drill (#185) waits on,
    /// exactly as the console reads it for a real deployment. `false` for an
    /// instance that does not exist yet, rather than an error -- a drill
    /// polling right after `apply` sees that before it sees anything else.
    fn is_ready<'a>(
        &'a self,
        reference: &'a IdentityInstanceRef,
    ) -> BoxFuture<'a, Result<bool, GenesisError>>;

    /// The connection string CloudNativePG generated for this instance's
    /// database, read from the secret it manages rather than assembled here
    /// -- the credentials are CloudNativePG's to mint, not genesis's to
    /// guess.
    fn database_uri<'a>(
        &'a self,
        reference: &'a IdentityInstanceRef,
    ) -> BoxFuture<'a, Result<String, GenesisError>>;

    /// Removes everything a drill (#185) created for itself, namespace
    /// included, in one call. A drill's namespace is dynamically named and
    /// holds nothing else, so deleting it is cheaper than deleting its
    /// resources one at a time and cannot leave one behind.
    fn delete_namespace<'a>(
        &'a self,
        namespace: &'a str,
    ) -> BoxFuture<'a, Result<(), GenesisError>>;
}

/// Creating and finding the upgrade resource the operator reconciles.
///
/// Finding is exposed rather than folded into a create-or-ignore, because
/// what to do about one that is already there is a decision, not a detail: a
/// replay of the same step is a no-op, and a different step is a refusal.
/// Deciding that here keeps it testable without a cluster.
pub trait IdentityInstanceUpgradePort: Send + Sync {
    fn find<'a>(
        &'a self,
        reference: &'a UpgradeRef,
    ) -> BoxFuture<'a, Result<Option<InFlightUpgrade>, GenesisError>>;

    fn create<'a>(&'a self, desired: &'a DesiredUpgrade)
    -> BoxFuture<'a, Result<(), GenesisError>>;
}

/// Drives the message-bus consumer loop.
pub trait EventConsumer: Send + Sync {
    /// Start consuming messages, dispatching each one to the registered handlers.
    fn run(&self) -> impl Future<Output = Result<(), GenesisError>> + Send;
}

/// Answers the one question a drill (#185) exists to ask: does the restored
/// database answer a query.
///
/// Its own port rather than a method on [`IdentityInstancePort`]: connecting
/// to the database a restore produced is not a Kubernetes resource, and a
/// port scoped to the CRD's lifecycle is not where a plain SQL connection
/// belongs.
pub trait DatabaseProbe: Send + Sync {
    fn answers_a_query<'a>(&'a self, uri: &'a str) -> BoxFuture<'a, Result<(), GenesisError>>;
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
