//! Taking an archive of an instance, and asking for one on a schedule.
//!
//! Two kinds rather than one, because they answer different questions and have
//! different lifetimes. A schedule is configuration that outlives every archive
//! it produces; a backup is one attempt that finishes.
//!
//! Named `IdentityInstanceBackup` rather than `Backup`. CloudNativePG already
//! owns `Backup` and `ScheduledBackup` on these same clusters, and two kinds
//! sharing a name costs somebody an hour every time they type
//! `kubectl get backup`. The prefix also says the right thing: this is a
//! backup of an instance, which is its database plus enough of its spec to
//! rebuild it, rather than a backup of a Postgres cluster.

use std::fmt::Display;

use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    common::types::{Condition, Phase},
    v1alpha::identity_instance_upgrade::IdentityInstanceRef,
};

/// How an archive is taken.
///
/// The names are CloudNativePG's, because these map one to one onto what it
/// does and inventing a second vocabulary for the same two things would mean
/// translating in both directions for ever.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveMethod {
    /// A base backup streamed to the object store, with WALs archived
    /// continuously alongside it.
    #[default]
    BarmanObjectStore,
    /// A snapshot of the volume, taken by the storage driver. Fast, and it
    /// lives in the provider's account rather than in the bucket.
    VolumeSnapshot,
}

impl Display for ArchiveMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BarmanObjectStore => write!(f, "barmanObjectStore"),
            Self::VolumeSnapshot => write!(f, "volumeSnapshot"),
        }
    }
}

#[derive(CustomResource, Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "aether.dev",
    version = "v1alpha",
    kind = "IdentityInstanceBackup",
    plural = "identityinstancebackups",
    shortname = "iib",
    namespaced,
    status = "IdentityInstanceBackupStatus",
    printcolumn = r#"{"name":"Instance", "type":"string", "jsonPath":".spec.identityInstanceRef.name"}"#,
    printcolumn = r#"{"name":"Method", "type":"string", "jsonPath":".spec.method"}"#,
    printcolumn = r#"{"name":"Phase", "type":"string", "jsonPath":".status.phase"}"#,
    printcolumn = r#"{"name":"Size", "type":"string", "jsonPath":".status.sizeBytes"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct IdentityInstanceBackupSpec {
    pub identity_instance_ref: IdentityInstanceRef,

    #[serde(default)]
    pub method: ArchiveMethod,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct IdentityInstanceBackupStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<Phase>,

    /// The CloudNativePG `Backup` doing the work. Recorded so an operator can
    /// follow one resource to the other without guessing at a naming scheme.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cnpg_backup_name: Option<String>,

    /// Where the archive went, as barman wrote it. What makes an archive
    /// findable without the control plane's database.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination_path: Option<String>,

    /// A string rather than a number: sizes pass a signed 32 bit integer
    /// somewhere in every Kubernetes toolchain, and a 3 GB archive read back
    /// as a negative number is worse than one nobody can sum.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<Time>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopped_at: Option<Time>,

    /// The Postgres major the archive was taken from. A base backup cannot be
    /// restored onto another one, and the archive is the only place that fact
    /// can still be read once the cluster is gone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postgres_major: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(CustomResource, Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "aether.dev",
    version = "v1alpha",
    kind = "IdentityInstanceBackupSchedule",
    plural = "identityinstancebackupschedules",
    shortname = "iibs",
    namespaced,
    status = "IdentityInstanceBackupScheduleStatus",
    printcolumn = r#"{"name":"Instance", "type":"string", "jsonPath":".spec.identityInstanceRef.name"}"#,
    printcolumn = r#"{"name":"Schedule", "type":"string", "jsonPath":".spec.schedule"}"#,
    printcolumn = r#"{"name":"Zone", "type":"string", "jsonPath":".spec.zone"}"#,
    printcolumn = r#"{"name":"Enabled", "type":"boolean", "jsonPath":".spec.enabled"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct IdentityInstanceBackupScheduleSpec {
    pub identity_instance_ref: IdentityInstanceRef,

    /// Six fields with seconds first, read in [`Self::zone`].
    ///
    /// No zone prefix. CloudNativePG's admission webhook counts whitespace
    /// separated fields and refuses anything but five or six, so
    /// `CRON_TZ=Europe/Paris 0 30 2 * * *` is rejected at apply time. That was
    /// found by applying one.
    ///
    /// The operator converts this to UTC before handing it on, and does so on
    /// every reconcile, so a daylight saving change corrects itself rather than
    /// leaving the schedule an hour out until somebody notices.
    pub schedule: String,

    /// The zone `schedule` is written in.
    ///
    /// A real zone rather than an offset: 02:30 local means 02:30 after a
    /// daylight saving change too, and an offset cannot say that.
    #[serde(default = "utc")]
    pub zone: String,

    #[serde(default)]
    pub method: ArchiveMethod,

    /// Whether the platform acts on it.
    ///
    /// A schedule that is off keeps its settings, so turning backups back on
    /// does not mean writing the cron out again from memory.
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

fn enabled_by_default() -> bool {
    true
}

fn utc() -> String {
    "UTC".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct IdentityInstanceBackupScheduleStatus {
    /// The CloudNativePG `ScheduledBackup` carrying this out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cnpg_scheduled_backup_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_scheduled_at: Option<Time>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
