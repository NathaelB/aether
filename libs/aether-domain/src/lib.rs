use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::{NoContext, Timestamp, Uuid};

use crate::dataplane::value_objects::DataPlaneId;

pub mod action;
pub mod audit;
pub mod catalog;
pub mod dataplane;
pub mod deployments;
pub mod metrics;
pub mod organisation;
pub mod role;
pub mod upgrades;
pub mod user;
pub mod version;

#[derive(Clone, Debug)]
pub struct AetherConfig {
    pub database: DatabaseConfig,
    pub auth: AuthConfig,
    pub dataplane: DataPlaneConfig,
}

#[derive(Clone, Copy, Debug)]
pub struct DataPlaneConfig {
    /// How long a data plane may go without reporting before placement stops
    /// selecting it.
    ///
    /// Should be a small multiple of Herald's poll interval: one missed cycle
    /// is a blip, three is a cluster that is gone. Too short turns a slow
    /// network into an outage; too long keeps sending deployments to a cluster
    /// that no longer exists.
    pub heartbeat_window: chrono::Duration,

    /// How long a data plane that has never reported is still believed to be
    /// coming up.
    ///
    /// `Provisioning` accepts dedicated placement on purpose -- it is the
    /// state every dedicated cluster passes through before its Herald reports.
    /// Without a bound, a provision that half-succeeded keeps accepting every
    /// deployment that organisation creates, each waiting on a Herald that is
    /// never coming.
    pub provisioning_timeout: chrono::Duration,

    /// How long a deployment whose tear-down was confirmed is kept.
    ///
    /// Its `actions` rows cascade with it, which is the whole reason this
    /// waits instead of removing the row at confirmation time: the history of
    /// what ran and who removed it outlives the deployment by a while.
    pub deleted_retention: chrono::Duration,
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

    /// A release only ever moves forward. Coming back from a withdrawal would
    /// mean a version that was pulled can be installed again.
    #[error("release {release} cannot go from {from} to {to}")]
    InvalidReleaseTransition {
        release: String,
        from: String,
        to: String,
    },

    /// The catalogue already holds this version of this product. Two rows for
    /// one release is what the unique index exists to prevent.
    #[error("release {release} is already in the catalogue")]
    ReleaseAlreadyExists { release: String },

    #[error("release {release} is not in the catalogue")]
    ReleaseNotFound { release: String },

    /// The catalogue holds it, and says it must not be installed. Withdrawn
    /// means exactly that, and an upgrade is an install.
    #[error("release {release} is {status} and cannot be installed")]
    ReleaseNotInstallable { release: String, status: String },

    #[error("deployment not found with id: {id}")]
    DeploymentNotFound { id: Uuid },

    /// An operation arrived while the deployment was being rewritten in place.
    /// Refused rather than queued: the caller can see the state and decide,
    /// and a queue would apply it at a moment nobody chose.
    #[error("deployment {deployment} is {status} and cannot be {operation} until that finishes")]
    DeploymentBusy {
        deployment: Uuid,
        status: String,
        operation: String,
    },

    /// Upgrading a deployment that is not settled would put two operations on
    /// one instance, and the second would win by accident.
    #[error("deployment {deployment} is {status} and cannot be upgraded")]
    DeploymentNotUpgradable { deployment: Uuid, status: String },

    /// An upgrade run's outcome is recorded once. Concluding it a second time
    /// would silently pick a winner between two true events -- the failure
    /// that happened and the retry that succeeded -- and this is the record
    /// that is not allowed to lose either one.
    #[error("upgrade run {run} already concluded as {outcome} and cannot be concluded again")]
    UpgradeRunAlreadyConcluded { run: Uuid, outcome: String },

    #[error("upgrade run not found with id: {id}")]
    UpgradeRunNotFound { id: Uuid },

    #[error(transparent)]
    Version(#[from] crate::version::VersionError),

    #[error("Invalid deployment resources: {reason}")]
    InvalidDeploymentResources { reason: String },

    #[error("Invalid data plane capacity")]
    InvalidDataPlaneCapacity,

    #[error("Data plane not found with id: {id}")]
    DataPlaneNotFound { id: DataPlaneId },

    /// The requested region is served, but every data plane in it is full,
    /// drained or unreachable. Retrying later may succeed.
    #[error("No data plane with room in region '{region}' for a {mode} deployment")]
    NoDataPlaneAvailable { region: String, mode: String },

    /// Nothing is deployed in the requested region at all. Retrying will not
    /// help, and the caller asked for something this installation cannot serve
    /// -- a different answer from "come back later".
    #[error("Region '{region}' has no data plane")]
    UnknownRegion { region: String },

    /// A dedicated deployment was asked for and no provisioner can create the
    /// cluster it needs.
    ///
    /// Distinct from `NoDataPlaneAvailable`, which means "come back later":
    /// retrying will not help, because nothing in this installation is able to
    /// make the infrastructure.
    #[error("{reason}")]
    ProvisioningUnavailable { reason: String },

    #[error("Permission denied: {reason}")]
    PermissionDenied { reason: String },

    // Repository errors
    #[error("Database error: {message}")]
    DatabaseError { message: String },

    #[error("Internal error: {0}")]
    InternalError(String),
}

pub fn generate_timestamp() -> (DateTime<Utc>, Timestamp) {
    let now = Utc::now();
    let seconds = now.timestamp().try_into().unwrap_or(0);
    let timestamp = Timestamp::from_unix(NoContext, seconds, 0);

    (now, timestamp)
}

pub fn generate_uuid_v7() -> Uuid {
    let (_, timestamp) = generate_timestamp();
    Uuid::new_v7(timestamp)
}
