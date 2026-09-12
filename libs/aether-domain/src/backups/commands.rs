use chrono::{DateTime, Utc};

use crate::{
    backups::{BackupMethod, PostgresMajor, keys::KeyRef},
    dataplane::value_objects::DataPlaneId,
    deployments::DeploymentId,
};

/// What a data plane says happened, once an archive exists.
///
/// Everything here was observed in the cluster. The control plane adds what it
/// already knows -- which organisation the deployment belongs to, and what
/// release it was running -- rather than trusting a report for facts it can
/// look up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordArchiveCommand {
    /// Which data plane is speaking. Checked against the deployment, because a
    /// Herald may only report about what runs on its own cluster.
    pub dataplane_id: DataPlaneId,
    pub deployment_id: DeploymentId,

    /// Relative to the deployment's archive prefix. The natural key of an
    /// archive: reporting the same one twice records it once.
    pub object_key: String,

    pub method: BackupMethod,
    pub postgres_major: PostgresMajor,
    pub key: KeyRef,

    /// Measured by whoever wrote the archive. Zero is refused rather than
    /// stored: an archive of no bytes is a backup that did not happen.
    pub size_bytes: u64,

    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

/// What a data plane says happened when an archive did not.
///
/// Deliberately carries almost nothing. There is no row to write and no size
/// to record; what is worth keeping is that an attempt was made, when, and
/// what it said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordArchiveFailureCommand {
    pub dataplane_id: DataPlaneId,
    pub deployment_id: DeploymentId,
    pub reason: String,
    pub attempted_at: DateTime<Utc>,
}
