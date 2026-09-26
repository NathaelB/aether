use std::future::Future;

use aether_auth::Identity;

use chrono::{DateTime, Utc};

use crate::{
    CoreError,
    backups::{
        ArchivePrefix, Backup, BackupId, BackupSchedule, BucketName, ObjectLocation,
        ObjectStoreError,
        commands::{
            AskForBackupCommand, RecordArchiveCommand, RecordArchiveFailureCommand,
            RecordDrillOutcomeCommand, SetBackupScheduleCommand,
        },
        keys::{DataKey, Dek, KeyError, KeyName, KeyRef, WrappedDek},
        restore::RestoreBackupCommand,
    },
    deployments::{Deployment, DeploymentId, cutover::CutoverCommand},
    organisation::OrganisationId,
    signals::Signal,
};

/// What the platform stores beside an archive, and reads back.
///
/// Deliberately small. The archive itself is written by CloudNativePG
/// straight from the data plane and never passes through this port; what
/// passes through is the metadata that has to survive losing the control
/// plane database, which is a few kilobytes.
///
/// Every operation is addressed through an [`ObjectLocation`], and those can
/// only be built from an [`ArchivePrefix`]. That is the whole tenant
/// isolation story at this layer: an implementation cannot be asked for a key
/// outside a prefix, so it does not have to check.
#[cfg_attr(test, mockall::automock)]
pub trait BackupStore: Send + Sync {
    fn put(
        &self,
        at: &ObjectLocation,
        body: Vec<u8>,
        content_type: &str,
    ) -> impl Future<Output = Result<(), ObjectStoreError>> + Send;

    /// `None` when the object is not there. Absence is an answer, not a
    /// failure: a deployment with no manifest yet is the normal state between
    /// a backup starting and finishing.
    fn get(
        &self,
        at: &ObjectLocation,
    ) -> impl Future<Output = Result<Option<Vec<u8>>, ObjectStoreError>> + Send;

    /// Everything under one deployment's prefix.
    ///
    /// Paginated by the implementation rather than by the caller. A
    /// deployment's archive count is bounded by its retention, so there is no
    /// cursor in this signature and no caller that would know what to do with
    /// one.
    fn list(
        &self,
        prefix: &ArchivePrefix,
    ) -> impl Future<Output = Result<Vec<ObjectLocation>, ObjectStoreError>> + Send;

    fn delete(
        &self,
        at: &ObjectLocation,
    ) -> impl Future<Output = Result<(), ObjectStoreError>> + Send;
}

/// Provisioning the bucket an installation archives into.
///
/// Separate from [`BackupStore`] because it is a different privilege. Writing
/// an archive is something every data plane does constantly; creating a
/// bucket is something that happens once, with credentials no tenant workload
/// ever holds. Splitting the ports is what keeps that difference visible in
/// the wiring instead of relying on nobody calling the wrong method.
pub trait BackupStoreAdmin: Send + Sync {
    /// Creates the bucket if it is not there, and applies the rules every
    /// bucket this platform writes to must carry.
    ///
    /// Idempotent, and called on every start rather than once at install: an
    /// installation that gained a bucket by hand, or lost its lifecycle rule
    /// to a console edit, is repaired without anybody noticing it was broken.
    fn ensure_bucket(
        &self,
        bucket: &BucketName,
    ) -> impl Future<Output = Result<(), ObjectStoreError>> + Send;
}

/// Where a data key comes from, and how it comes back.
///
/// Shaped on wrapping and unwrapping a small payload plus generating a data
/// key, which is what Vault Transit does, what OpenBao does, and what every
/// cloud key manager does. A port shaped on anything else would have to be
/// redesigned at the first real one instead of gaining an adapter.
pub trait KeyProvider: Send + Sync {
    /// A fresh data key, returned in the clear and wrapped at once.
    ///
    /// The plaintext exists only here. A caller that drops it goes back
    /// through [`KeyProvider::unwrap_data_key`], which is the cost this
    /// signature makes visible rather than hiding behind a cache.
    fn generate_data_key(
        &self,
        name: &KeyName,
    ) -> impl Future<Output = Result<DataKey, KeyError>> + Send;

    /// The key an archive was encrypted under, from what the archive recorded.
    ///
    /// Fails with [`KeyError::KeyUnavailable`] when the key is destroyed or
    /// revoked, and with [`KeyError::ProviderUnavailable`] when nobody
    /// answered. Those are not the same incident and the caller is expected to
    /// act on them differently.
    fn unwrap_data_key(
        &self,
        key: &KeyRef,
        wrapped: &WrappedDek,
    ) -> impl Future<Output = Result<Dek, KeyError>> + Send;
}

/// Creating the key an installation wraps with.
///
/// Separate from [`KeyProvider`] for the same reason [`BackupStoreAdmin`] is
/// separate from [`BackupStore`]: generating a data key happens on every
/// backup, creating a key happens once, and the credentials are not the same.
pub trait KeyProviderAdmin: Send + Sync {
    /// Creates the key if it is not there, and the engine holding it if that
    /// is not there either.
    ///
    /// Idempotent, and called on every start. An installation whose key
    /// manager was rebuilt comes back without anybody noticing it was gone,
    /// and one whose key was destroyed does not silently get a new one under
    /// the same name: creating a key that already exists is a no-op, and this
    /// never deletes.
    fn ensure_key(&self, name: &KeyName) -> impl Future<Output = Result<(), KeyError>> + Send;
}

/// Archives that exist.
///
/// There is no method to record a failure, and that absence is the port's main
/// statement. An attempt that did not finish goes to `actions` and the audit
/// log through the ports those contexts already own.
#[cfg_attr(test, mockall::automock)]
pub trait BackupRepository: Send + Sync {
    fn record(&self, backup: Backup) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn get(&self, id: &BackupId) -> impl Future<Output = Result<Option<Backup>, CoreError>> + Send;

    /// The archive at this key, if it is already recorded.
    ///
    /// The object key is an archive's natural identity: one object, one
    /// archive. Reports arrive at least once by design, so this is what stops
    /// a redelivery being counted twice by retention and offered twice as a
    /// restore.
    fn find_by_object_key(
        &self,
        deployment: &DeploymentId,
        object_key: &str,
    ) -> impl Future<Output = Result<Option<Backup>, CoreError>> + Send;

    /// One deployment's archives, newest first.
    ///
    /// The whole set rather than a page. Retention cannot answer "is this among
    /// the newest seven" about an archive on its own, and a deployment's
    /// archive count is bounded by its own retention, so there is no caller
    /// that would know what to do with a cursor.
    fn list_for_deployment(
        &self,
        deployment: &DeploymentId,
    ) -> impl Future<Output = Result<Vec<Backup>, CoreError>> + Send;

    /// Forgets an archive the platform has already removed from the store.
    ///
    /// In that order, deliberately. A row removed before its object leaves an
    /// object nothing will ever delete; an object removed before its row leaves
    /// a row promising a restore that cannot happen, and the second is
    /// recoverable by deleting the row.
    fn forget(&self, id: &BackupId) -> impl Future<Output = Result<(), CoreError>> + Send;
}

#[cfg_attr(test, mockall::automock)]
pub trait BackupScheduleRepository: Send + Sync {
    /// Writes the schedule, replacing whatever was there.
    ///
    /// One schedule per deployment, so this is an upsert rather than an insert
    /// that can collide. Two schedules would be two answers to "when is this
    /// backed up".
    fn save(&self, schedule: BackupSchedule) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn get(
        &self,
        deployment: &DeploymentId,
    ) -> impl Future<Output = Result<Option<BackupSchedule>, CoreError>> + Send;

    /// Every schedule the platform should be acting on.
    fn list_enabled(&self) -> impl Future<Output = Result<Vec<BackupSchedule>, CoreError>> + Send;
}

/// Who may look at a deployment's archives, and who may change them.
pub trait BackupPolicy: Send + Sync {
    fn can_view_backups(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn can_manage_backups(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;
}

/// Information about a potential backup signal that needs to be opened or closed.
#[derive(Debug, Clone)]
pub struct BackupSignalUpdate {
    pub deployment_id: DeploymentId,
    pub dedup_key_prefix: String,
    pub signal_to_open: Option<Signal>,
    pub should_close: bool,
}

/// What the API layer calls when a data plane reports an archive.
///
/// A trait rather than a concrete service for the same reason every other
/// context has one: the transaction and the repositories are assembled in the
/// application layer, and the handler should not know that either exists.
pub trait BackupService: Send + Sync {
    /// Records an archive. Idempotent on the object key, because reports are
    /// at-least-once and an archive recorded twice would be counted twice by
    /// retention and offered twice as a restore.
    fn record_archive(
        &self,
        identity: Identity,
        command: RecordArchiveCommand,
    ) -> impl Future<Output = Result<Backup, CoreError>> + Send;

    /// Records that an archive was attempted and did not happen. Writes an
    /// audit entry and no backup row.
    fn record_archive_failure(
        &self,
        identity: Identity,
        command: RecordArchiveFailureCommand,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    /// One deployment's archives, newest first.
    fn list_backups(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> impl Future<Output = Result<Vec<Backup>, CoreError>> + Send;

    /// When this deployment is archived, and how much history is kept.
    ///
    /// Answers with the default rather than with nothing when no row exists.
    /// A deployment created before the platform wrote schedules is archived on
    /// the same terms as one created after it, and a screen showing "none"
    /// would be describing the row rather than the deployment.
    fn get_backup_schedule(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> impl Future<Output = Result<BackupSchedule, CoreError>> + Send;

    fn set_backup_schedule(
        &self,
        identity: Identity,
        command: SetBackupScheduleCommand,
    ) -> impl Future<Output = Result<BackupSchedule, CoreError>> + Send;

    /// Asks for an archive now, rather than waiting for the schedule.
    ///
    /// Answers with nothing. The archive does not exist when this returns --
    /// the data plane has been told to take one, and it appears in the
    /// deployment's list when it is reported. A response carrying an archive
    /// would be a lie with a shape.
    fn ask_for_backup(
        &self,
        identity: Identity,
        command: AskForBackupCommand,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    /// Brings an archive back as a second deployment.
    ///
    /// Returns the recovery, not the source. Nothing about the source changes:
    /// it keeps its name, its hostname and its traffic, and the caller decides
    /// afterwards whether anything moves. That decision is a separate act.
    fn restore_backup(
        &self,
        identity: Identity,
        command: RestoreBackupCommand,
    ) -> impl Future<Output = Result<Deployment, CoreError>> + Send;

    /// Moves a hostname from one deployment to the other -- the act that
    /// actually ends an outage, once a recovery has been inspected and
    /// trusted.
    ///
    /// Refused for any pair that is not an actual restore of one from the
    /// other: a cutover between unrelated deployments would hand a
    /// customer's hostname to a deployment nothing ever archived from it.
    ///
    /// Symmetric: called with `command.promote`/`command.demote` reversed,
    /// this is a cutback rather than a second operation.
    fn cutover(
        &self,
        identity: Identity,
        command: CutoverCommand,
    ) -> impl Future<Output = Result<(Deployment, Deployment), CoreError>> + Send;

    /// Every deployment a drill (#185) may run against right now, unattended.
    ///
    /// Scoped to deployments that actually archive: a drill restores an
    /// archive, and one that has none has nothing to prove. Everything else
    /// -- the window, the interval since the last one -- is decided by
    /// [`crate::backups::drill_schedule::consider`].
    fn deployments_due_for_drill(
        &self,
    ) -> impl Future<Output = Result<Vec<DeploymentId>, CoreError>> + Send;

    /// Asks a deployment's own data plane to restore its latest archive into
    /// a throwaway namespace, verify it, and tear it down (#185).
    ///
    /// No `Identity`: this is triggered by the scheduler on a deployment's
    /// maintenance window, the same way [`crate::backups::ports::BackupRepository`]'s
    /// callers trigger a scheduled archive -- not a request a caller makes
    /// about their own deployment.
    fn trigger_drill(
        &self,
        deployment_id: DeploymentId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    /// Records what a drill (#185) proved, or did not: a fact on the
    /// deployment when it succeeded, an audit entry and nothing else when it
    /// failed.
    fn record_drill_outcome(
        &self,
        identity: Identity,
        command: RecordDrillOutcomeCommand,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    /// Finds deployments that need backup signals opened or closed.
    ///
    /// No `Identity`: this is called by the backup signal probe, which runs
    /// on a schedule the same way `deployments_due_for_drill` and `trigger_drill`
    /// do.
    fn find_backup_signals(
        &self,
        now: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<BackupSignalUpdate>, CoreError>> + Send;
}
