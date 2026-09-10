use thiserror::Error;

#[derive(Debug, Error)]
pub enum GenesisError {
    #[error("Failed to deserialize event: {message}")]
    Deserialization { message: String },

    #[error("Message bus error: {message}")]
    MessageBus { message: String },

    #[error("Handler error: {message}")]
    Handler { message: String },

    #[error("Invalid deployment payload: {message}")]
    InvalidPayload { message: String },

    #[error("Kubernetes error: {message}")]
    Kubernetes { message: String },

    /// A different upgrade is already on this deployment. Refused rather than
    /// replaced: the one in flight may already have patched the instance, and
    /// pointing it somewhere else mid-step is how a version nobody asked for
    /// ends up running.
    #[error("deployment {deployment_id} is already upgrading to {in_flight}, not {requested}")]
    UpgradeAlreadyInFlight {
        deployment_id: uuid::Uuid,
        in_flight: String,
        requested: String,
    },

    #[error("Internal error: {message}")]
    Internal { message: String },
}
