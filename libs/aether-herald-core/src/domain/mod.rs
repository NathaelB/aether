pub mod action;
pub mod dataplane;
pub mod deployment;
pub mod ports;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum HeraldError {
    #[error("Invalid action: {message}")]
    InvalidAction { message: String },

    #[error("Control plane error: {message}")]
    ControlPlane { message: String },

    #[error("Message bus error: {message}")]
    MessageBus { message: String },

    #[error("Internal error: {message}")]
    Internal { message: String },
}
