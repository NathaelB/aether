use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum HeraldError {
    #[error("Invalid action: {message}")]
    InvalidAction { message: String },

    #[error("Control plane error: {message}")]
    ControlPlane { message: String },

    #[error("Message bus error: {message}")]
    MessageBus { message: String },

    /// An IAM instance could not be read. Distinct from every other variant
    /// because it is the one the collector is required to treat as "unknown"
    /// rather than as "nothing happened".
    #[error("Usage source error: {message}")]
    UsageSource { message: String },

    #[error("Internal error: {message}")]
    Internal { message: String },
}
