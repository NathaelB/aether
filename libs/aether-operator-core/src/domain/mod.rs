pub mod identity_instance;
pub mod ports;

use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Clone, Default)]
pub struct ReconcileOutcome {
    pub requeue_after: Option<Duration>,
}

impl ReconcileOutcome {
    pub fn requeue_after(duration: Duration) -> Self {
        Self {
            requeue_after: Some(duration),
        }
    }
}

#[derive(Debug, Error)]
pub enum OperatorError {
    #[error("IdentityInstance is missing metadata.name")]
    MissingName,

    #[error("IdentityInstance {name} is missing metadata.namespace")]
    MissingNamespace { name: String },

    #[error("Kubernetes API error: {message}")]
    Kube { message: String },

    #[error("Internal operator error: {message}")]
    Internal { message: String },
}
