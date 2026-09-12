use std::future::Future;

use crate::backups::{ArchivePrefix, BucketName, ObjectLocation, ObjectStoreError};

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

    fn delete(&self, at: &ObjectLocation)
    -> impl Future<Output = Result<(), ObjectStoreError>> + Send;
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
