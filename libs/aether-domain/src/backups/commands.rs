use chrono::{DateTime, Utc};

use chrono_tz::Tz;

use crate::{
    backups::{ArchiveProtection, BackupMethod, Cadence, PostgresMajor, Retention},
    dataplane::value_objects::DataPlaneId,
    deployments::DeploymentId,
    organisation::OrganisationId,
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

    /// The name barman filed the archive under, as the data plane observed it.
    ///
    /// `None` for an archive taken before data planes reported it. Such an
    /// archive cannot be restored from -- the recovery would have to be told
    /// where to read, and guessing is how one comes up empty looking restored.
    pub server_name: Option<String>,

    /// What protects the archive. A data plane taking an operational backup
    /// reports the store's own encryption; nothing wraps a key for it, and
    /// naming one anyway would record a key that unwraps nothing.
    pub protection: ArchiveProtection,

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

/// What a drill (#185) proved, or did not.
///
/// Not symmetrical, for the same reason [`RecordArchiveFailureCommand`]
/// exists apart from a successful archive: a drill that succeeded is a fact
/// about the deployment worth keeping on it, and a drill that failed is an
/// attempt worth keeping in the audit trail and nowhere else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrillOutcome {
    Succeeded {
        duration_seconds: u64,
    },
    Failed {
        reason: String,
        duration_seconds: Option<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordDrillOutcomeCommand {
    pub dataplane_id: DataPlaneId,
    pub deployment_id: DeploymentId,
    pub outcome: DrillOutcome,
    pub observed_at: DateTime<Utc>,
}

impl RecordDrillOutcomeCommand {
    pub fn parse(
        dataplane_id: DataPlaneId,
        deployment_id: DeploymentId,
        outcome: &str,
        duration_seconds: Option<u64>,
        reason: Option<String>,
        observed_at: DateTime<Utc>,
    ) -> Result<Self, String> {
        let outcome = match outcome {
            "drill_succeeded" => DrillOutcome::Succeeded {
                duration_seconds: duration_seconds
                    .ok_or_else(|| "a succeeded drill must report how long it took".to_string())?,
            },
            "drill_failed" => DrillOutcome::Failed {
                reason: reason.ok_or_else(|| "a failed drill must say why".to_string())?,
                duration_seconds,
            },
            other => {
                return Err(format!(
                    "unknown drill outcome '{other}', expected one of: drill_succeeded, drill_failed"
                ));
            }
        };

        Ok(Self {
            dataplane_id,
            deployment_id,
            outcome,
            observed_at,
        })
    }
}

/// What a customer asks for when they change how their data is protected.
///
/// Every field is present rather than optional. A schedule is read as a whole
/// -- cadence, zone, retention and whether it runs at all -- and a partial
/// update would leave a screen unable to say what the deployment is actually
/// on without reading back what it did not send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetBackupScheduleCommand {
    pub organisation_id: OrganisationId,
    pub deployment_id: DeploymentId,
    pub cadence: Cadence,
    pub zone: Tz,
    pub retention: Retention,
    pub enabled: bool,
}

/// Asking for an archive now, rather than waiting for the schedule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskForBackupCommand {
    pub organisation_id: crate::organisation::OrganisationId,
    pub deployment_id: crate::deployments::DeploymentId,
    pub requested_by: crate::user::UserId,
}
