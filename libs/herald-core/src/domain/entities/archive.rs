//! What the cluster says about an archive, on its way to the control plane.
//!
//! Herald is a courier here, not an author. It did not take the archive and it
//! does not decide what one is: it reads what the operator recorded and says
//! it upward, with the credentials the operator does not hold.

use chrono::{DateTime, Utc};

use crate::domain::entities::deployment::DeploymentId;

/// One archive, as the data plane observed it.
///
/// Either it exists and every field describing it is present, or it does not
/// and only the reason is. The two are one type because they are the two
/// outcomes of one attempt, and the control plane tells them apart the same
/// way -- by which fields arrived, not by a flag somebody can set
/// inconsistently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Archive {
    Taken(TakenArchive),
    Failed(FailedArchive),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TakenArchive {
    pub deployment_id: DeploymentId,

    /// Relative to the deployment's own prefix, and the archive's identity:
    /// reports are at-least-once, and this is what makes a redelivery record
    /// the same archive rather than a second one.
    pub object_key: String,

    /// The Postgres major that wrote it. A base backup does not restore onto
    /// another one, and once the cluster is gone the archive is the only place
    /// this can still be read.
    pub postgres_major: u32,

    /// A string, because a size passes through a signed 32 bit integer
    /// somewhere in every Kubernetes toolchain and a 3 GB archive read back as
    /// a negative number is worse than one nobody can sum.
    pub size_bytes: String,

    /// The name barman filed it under, as CloudNativePG reported it.
    ///
    /// Carried because a recovery has to ask for it, and the control plane
    /// used to derive it -- which made a `deployment-<uuid>-db` convention
    /// something three implementations had to agree on, with a failure nobody
    /// sees when they do not.
    ///
    /// `None` for an archive taken by a data plane older than this.
    pub server_name: Option<String>,

    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedArchive {
    pub deployment_id: DeploymentId,
    pub reason: String,
}

impl Archive {
    pub fn deployment_id(&self) -> &DeploymentId {
        match self {
            Self::Taken(taken) => &taken.deployment_id,
            Self::Failed(failed) => &failed.deployment_id,
        }
    }
}
