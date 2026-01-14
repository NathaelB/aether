use thiserror::Error;
use uuid::Uuid;

pub mod action;
pub mod auth;
pub mod deployments;
pub mod organisation;
pub mod policy;
pub mod role;
pub mod user;

#[derive(Clone, Debug)]
pub struct AetherConfig {
    pub database: DatabaseConfig,
    pub auth: AuthConfig,
}

#[derive(Clone, Debug)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct AuthConfig {
    pub issuer: String,
}

#[derive(Debug, Error)]
pub enum CoreError {
    // Organisation errors
    #[error("Organisation '{organisation_name}' creation failed: {reason}")]
    FailedCreateOrganisation {
        organisation_name: String,
        reason: String,
    },

    #[error("Invalid organisation name: {reason}")]
    InvalidOrganisationName { reason: String },

    #[error("Invalid organisation slug: {reason}")]
    InvalidOrganisationSlug { reason: String },

    #[error("Organisation not found with id: {id}")]
    OrganisationNotFound { id: Uuid },

    #[error("Organisation not found with slug: {slug}")]
    OrganisationNotFoundBySlug { slug: String },

    #[error("Organisation slug '{slug}' is already taken")]
    OrganisationSlugAlreadyExists { slug: String },

    #[error("Organisation is suspended: {reason}")]
    OrganisationSuspended { reason: String },

    #[error("Organisation limit reached: {limit_type} (max: {max}, current: {current})")]
    OrganisationLimitReached {
        limit_type: String,
        max: usize,
        current: usize,
    },

    #[error("User has reached maximum number of organisations (max: {max}, current: {current})")]
    UserOrganisationLimitReached { max: usize, current: usize },

    #[error("Invalid organisation status: {value}")]
    InvalidOrganisationStatus { value: String },

    #[error("Invalid plan: {value}")]
    InvalidPlan { value: String },

    #[error("Invalid identity")]
    InvalidIdentity,

    #[error("Permission denied: {reason}")]
    PermissionDenied { reason: String },

    // Repository errors
    #[error("Database error: {message}")]
    DatabaseError { message: String },

    #[error("Internal error: {0}")]
    InternalError(String),
}
