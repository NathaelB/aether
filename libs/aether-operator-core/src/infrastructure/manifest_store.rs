//! The operator's window onto the object store.
//!
//! The only thing in the data plane that talks to it directly. Everything else
//! about an archive is CloudNativePG's, so this stays small: one PUT of a few
//! kilobytes for the manifest, and one LIST to find out what the archive
//! weighs -- which CloudNativePG does not say and the control plane will not
//! record an archive without.

use std::time::Duration;

use aws_config::{BehaviorVersion, Region};
use aws_credential_types::Credentials;
use aws_sdk_s3::{
    Client,
    config::{Builder, timeout::TimeoutConfig},
    primitives::ByteStream,
};
use tracing::{info, warn};

use crate::{
    domain::OperatorError,
    infrastructure::{
        archive::{ArchiveStore, split_destination},
        identity_instance_backup::ArchiveObjects,
    },
};

/// Bounded for the same reason the control plane's client is: a store that has
/// not answered in this long is not busy, it is somewhere else.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(30);

pub struct S3ManifestWriter {
    client: Client,
}

impl S3ManifestWriter {
    pub fn new(store: &ArchiveStore) -> Self {
        let credentials = Credentials::new(
            store.access_key_id.clone(),
            store.secret_access_key.clone(),
            None,
            None,
            "aether-operator",
        );

        let mut builder = Builder::new()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(store.region.clone()))
            .credentials_provider(credentials)
            // host/bucket. Every store a data plane writes to is reached this
            // way, and AWS accepts it too.
            .force_path_style(true)
            .timeout_config(
                TimeoutConfig::builder()
                    .connect_timeout(CONNECT_TIMEOUT)
                    .operation_attempt_timeout(ATTEMPT_TIMEOUT)
                    .build(),
            );

        if let Some(endpoint) = &store.endpoint_url {
            builder = builder.endpoint_url(endpoint);
        }

        Self {
            client: Client::from_conf(builder.build()),
        }
    }
}

#[async_trait::async_trait]
impl ArchiveObjects for S3ManifestWriter {
    async fn write(
        &self,
        destination: &str,
        object_key: &str,
        body: Vec<u8>,
    ) -> Result<(), OperatorError> {
        let (bucket, prefix) =
            split_destination(destination).ok_or_else(|| OperatorError::Configuration {
                message: format!("'{destination}' is not an S3 destination"),
            })?;

        let key = format!("{prefix}/{object_key}");

        self.client
            .put_object()
            .bucket(bucket)
            .key(&key)
            .content_type("application/json")
            .body(ByteStream::from(body))
            .send()
            .await
            .map_err(|error| OperatorError::Internal {
                message: format!("the manifest could not be written to {bucket}/{key}: {error}"),
            })?;

        info!(bucket, key, "the archive carries a manifest");
        Ok(())
    }

    async fn measure(&self, destination: &str, prefix: &str) -> Option<u64> {
        let (bucket, root) = split_destination(destination)?;
        let under = format!("{root}/{prefix}/");

        // Paginated, because an archive is many objects and a store answers a
        // thousand at a time. A total that silently stopped at the first page
        // would be a size, which is worse than none.
        let mut total: u64 = 0;
        let mut pages = self
            .client
            .list_objects_v2()
            .bucket(bucket)
            .prefix(&under)
            .into_paginator()
            .send();

        while let Some(page) = pages.next().await {
            match page {
                Ok(page) => {
                    total += page
                        .contents()
                        .iter()
                        .filter_map(|object| object.size())
                        .map(|size| size.max(0) as u64)
                        .sum::<u64>();
                }
                Err(error) => {
                    // Reported, not propagated. A size nobody could measure
                    // must not fail the reconcile that was recording an
                    // archive which does exist.
                    warn!(bucket, prefix = %under, %error, "the archive could not be measured");
                    return None;
                }
            }
        }

        info!(bucket, prefix = %under, total, "the archive was measured");

        // Zero is not a small archive. An empty prefix means the objects are
        // not where this expected them, and reporting it would record a backup
        // that did not happen.
        (total > 0).then_some(total)
    }
}
