//! Making sure the bucket archives go into exists, and carries the rules every
//! bucket this platform writes to must carry.
//!
//! Done by the control plane rather than by a data plane, and that is not a
//! contradiction of who writes archives. A data plane writes into a prefix
//! with credentials scoped to it; creating the bucket and setting its
//! lifecycle needs credentials no tenant workload ever holds, and happens
//! once. Splitting those two is the point.

use std::{sync::Arc, time::Duration};

use aether_core::backups::{BucketName, ports::BackupStoreAdmin};
use aether_s3::{ObjectStoreConfig, S3ObjectStore, STARTUP_GRACE};
use tracing::{error, info, warn};

use crate::args::{Args, ObjectStoreArgs};

/// How long to wait between attempts while the store is still coming up.
const RETRY_INTERVAL: Duration = Duration::from_secs(3);

impl ObjectStoreArgs {
    /// `None` when archiving is switched off, which is what an empty bucket
    /// name means. An empty endpoint means AWS, and those two are different
    /// questions that a single "is it configured" flag would conflate.
    pub fn config(
        &self,
    ) -> Option<Result<ObjectStoreConfig, aether_core::backups::ObjectStoreError>> {
        if self.bucket.trim().is_empty() {
            return None;
        }

        Some(
            BucketName::new(self.bucket.clone()).map(|bucket| ObjectStoreConfig {
                endpoint: self.endpoint.clone(),
                region: self.region.clone(),
                access_key_id: self.access_key_id.clone(),
                secret_access_key: self.secret_access_key.clone(),
                force_path_style: self.force_path_style,
                bucket,
            }),
        )
    }
}

/// Creates the archive bucket if it is not there, retrying while the store
/// comes up alongside us.
///
/// Never blocks the control plane from serving. A store that is unreachable
/// stops backups, not authentication, and refusing to start would turn one
/// outage into two. It is loud instead: an installation that archives nowhere
/// has to be visible in the logs from the first line, because the alternative
/// is finding out during a restore.
pub async fn ensure_archive_bucket(args: Arc<Args>) {
    ensure_archive_bucket_within(args, STARTUP_GRACE).await
}

/// [`ensure_archive_bucket`] with the window it keeps retrying in spelled out.
///
/// Exists so a test can assert what happens when the store never comes up
/// without waiting the real grace to find out. A timeout that cannot be
/// shortened is a timeout nothing covers.
pub async fn ensure_archive_bucket_within(args: Arc<Args>, grace: Duration) {
    let config = match args.object_store.config() {
        None => {
            warn!("no archive bucket is configured: this installation backs up nothing");
            return;
        }
        Some(Err(error)) => {
            error!(%error, "the configured archive bucket cannot be addressed");
            return;
        }
        Some(Ok(config)) => config,
    };

    let bucket = config.bucket.clone();
    let store = S3ObjectStore::new(config);
    let deadline = tokio::time::Instant::now() + grace;

    loop {
        match store.ensure_bucket(&bucket).await {
            Ok(()) => {
                info!(bucket = bucket.as_str(), "archives have somewhere to go");
                return;
            }
            Err(error) if tokio::time::Instant::now() < deadline => {
                info!(bucket = bucket.as_str(), %error, "the object store is not up yet, trying again");
                tokio::time::sleep(RETRY_INTERVAL).await;
            }
            Err(error) => {
                error!(
                    bucket = bucket.as_str(),
                    %error,
                    "the archive bucket could not be reached: nothing will be backed up until it can"
                );
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_bucket_name_switches_archiving_off() {
        let args = ObjectStoreArgs {
            bucket: "  ".to_string(),
            ..ObjectStoreArgs::default()
        };

        assert!(args.config().is_none());
    }

    #[test]
    fn an_absent_endpoint_still_configures_a_store() {
        let args = ObjectStoreArgs {
            endpoint: None,
            ..ObjectStoreArgs::default()
        };

        let config = args
            .config()
            .expect("archiving is on")
            .expect("the bucket is valid");
        assert!(config.endpoint.is_none());
        assert_eq!(config.bucket.as_str(), "aether-backups");
    }

    #[test]
    fn a_bucket_name_no_store_would_accept_is_reported_rather_than_used() {
        let args = ObjectStoreArgs {
            bucket: "Not A Bucket".to_string(),
            ..ObjectStoreArgs::default()
        };

        assert!(args.config().expect("archiving is on").is_err());
    }
}
