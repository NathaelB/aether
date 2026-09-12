//! Writing the manifest beside an archive.
//!
//! The only thing in the data plane that talks to the object store directly.
//! Everything else about an archive is CloudNativePG's, which is why this is
//! forty lines rather than a client library: one PUT of a few kilobytes, and a
//! failure that is reported and not propagated.

use std::time::Duration;

use aws_config::{BehaviorVersion, Region};
use aws_credential_types::Credentials;
use aws_sdk_s3::{
    Client,
    config::{Builder, timeout::TimeoutConfig},
    primitives::ByteStream,
};
use tracing::info;

use crate::{
    domain::OperatorError,
    infrastructure::{
        archive::{ArchiveStore, split_destination},
        identity_instance_backup::ManifestWriter,
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
impl ManifestWriter for S3ManifestWriter {
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
}
