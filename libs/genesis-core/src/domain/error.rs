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

    #[error("Internal error: {message}")]
    Internal { message: String },
}
