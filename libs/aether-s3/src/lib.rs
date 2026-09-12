//! The object store adapter, against anything that speaks S3.
//!
//! One adapter for RustFS locally, Scaleway in production and a customer's own
//! bucket later. The three disagree on endpoint, region and addressing style,
//! and on nothing else that matters here, so all three are configuration
//! rather than code paths.

use std::time::Duration;

use aether_domain::backups::{
    ArchivePrefix, BucketName, INCOMPLETE_UPLOAD_GRACE_DAYS, ObjectLocation, ObjectStoreError,
    ports::{BackupStore, BackupStoreAdmin},
};
use aws_config::{BehaviorVersion, Region};
use aws_credential_types::Credentials;
use aws_sdk_s3::{
    Client,
    config::{Builder, timeout::TimeoutConfig},
    error::SdkError,
    operation::put_object::builders::PutObjectFluentBuilder,
    primitives::ByteStream,
    types::{
        AbortIncompleteMultipartUpload, BucketLifecycleConfiguration, ExpirationStatus,
        LifecycleRule, LifecycleRuleFilter, ServerSideEncryption,
    },
};
use tracing::{info, warn};

/// Everything that differs between one S3 implementation and the next.
#[derive(Debug, Clone)]
pub struct ObjectStoreConfig {
    /// Where the store answers. Set for RustFS and for any provider that is
    /// not AWS; left empty only when talking to AWS itself.
    pub endpoint: Option<String>,

    /// Signed into every request. Non AWS stores mostly ignore which region
    /// it is and mind very much that there is one, so this has no default
    /// that silently works in one place and not another.
    pub region: String,

    pub access_key_id: String,
    pub secret_access_key: String,

    /// `bucket.host` against AWS, `host/bucket` against most self hosted
    /// stores. Getting it wrong produces a DNS failure that names the bucket,
    /// which reads like a permissions problem and is not one.
    pub force_path_style: bool,

    pub bucket: BucketName,

    /// What the store itself does to an object once it has it.
    ///
    /// Worth having and worth not overstating. The store decrypts on read, so
    /// this protects the disks under the bucket and the operator of the
    /// provider is not locked out by it. An archive nobody but the customer
    /// can read is a different mechanism, encrypted before it leaves the data
    /// plane, and it is a separate chantier.
    pub encryption: StoreEncryption,
}

/// How the object store is asked to encrypt what it is given.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StoreEncryption {
    /// The store's own key. Every store here supports it, and it is the
    /// default because a bucket writing archives in the clear should be
    /// something somebody chose rather than something they forgot.
    #[default]
    Managed,

    /// A key in the provider's key manager. Narrows who at the provider can
    /// read the bucket; the provider still performs the decryption.
    ProviderKey { key_id: String },

    /// Nothing. For a store that does not implement any of it, which is the
    /// only reason to pick this.
    None,
}

impl StoreEncryption {
    /// Parsed from configuration: `managed`, `none`, or `kms:<key id>`.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "" | "managed" | "aes256" => Ok(Self::Managed),
            "none" | "off" => Ok(Self::None),
            other => match other.strip_prefix("kms:") {
                Some(key_id) if !key_id.is_empty() => Ok(Self::ProviderKey {
                    key_id: key_id.to_string(),
                }),
                _ => Err(format!(
                    "'{other}' is not a store encryption mode: use managed, none, or kms:<key id>"
                )),
            },
        }
    }

    fn apply(&self, request: PutObjectFluentBuilder) -> PutObjectFluentBuilder {
        match self {
            Self::Managed => request.server_side_encryption(ServerSideEncryption::Aes256),
            Self::ProviderKey { key_id } => request
                .server_side_encryption(ServerSideEncryption::AwsKms)
                .ssekms_key_id(key_id),
            Self::None => request,
        }
    }
}

/// Reads an archive's metadata, writes it back, and knows nothing about
/// tenants beyond the prefix it is handed.
#[derive(Debug, Clone)]
pub struct S3ObjectStore {
    client: Client,
    bucket: BucketName,
    encryption: StoreEncryption,
}

impl S3ObjectStore {
    pub fn new(config: ObjectStoreConfig) -> Self {
        let credentials = Credentials::new(
            config.access_key_id,
            config.secret_access_key,
            None,
            None,
            "aether-object-store",
        );

        let mut builder = Builder::new()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(config.region))
            .credentials_provider(credentials)
            .force_path_style(config.force_path_style)
            // Bounded on purpose. The SDK waits a long time by default for a
            // host that never answers, which is right for a transfer and wrong
            // for a store that is simply not there: a misconfigured endpoint
            // should say so in seconds rather than look like a slow network
            // for half a minute.
            .timeout_config(
                TimeoutConfig::builder()
                    .connect_timeout(CONNECT_TIMEOUT)
                    .operation_attempt_timeout(ATTEMPT_TIMEOUT)
                    .build(),
            );

        if let Some(endpoint) = config.endpoint {
            builder = builder.endpoint_url(endpoint);
        }

        Self {
            client: Client::from_conf(builder.build()),
            bucket: config.bucket,
            encryption: config.encryption,
        }
    }

    pub fn bucket(&self) -> &BucketName {
        &self.bucket
    }

    pub fn encryption(&self) -> &StoreEncryption {
        &self.encryption
    }

    /// The underlying client, for the operations this port deliberately does
    /// not carry: presigning, multipart, and reading back what
    /// [`BackupStoreAdmin::ensure_bucket`] applied. Widening the port for
    /// those would put store specific concepts in the domain.
    pub fn client(&self) -> &Client {
        &self.client
    }
}

/// Turns an SDK failure into something a caller can act on.
///
/// The distinction that matters is whether the store answered. A refusal is
/// reported to whoever asked; an unreachable store is retried. Collapsing the
/// two into one error means every transport blip looks like a permissions
/// problem, and every permissions problem gets retried until it times out.
fn classify<E, R>(operation: &str, location: &str, error: SdkError<E, R>) -> ObjectStoreError
where
    E: std::fmt::Debug,
{
    match error {
        SdkError::ServiceError(inner) => ObjectStoreError::Refused {
            operation: operation.to_string(),
            location: location.to_string(),
            reason: format!("{:?}", inner.err()),
        },
        other => ObjectStoreError::Unavailable {
            operation: operation.to_string(),
            location: location.to_string(),
            reason: other.to_string(),
        },
    }
}

impl BackupStore for S3ObjectStore {
    async fn put(
        &self,
        at: &ObjectLocation,
        body: Vec<u8>,
        content_type: &str,
    ) -> Result<(), ObjectStoreError> {
        let path = at.as_path();

        let request = self
            .client
            .put_object()
            .bucket(self.bucket.as_str())
            .key(&path)
            .content_type(content_type)
            .body(ByteStream::from(body));

        self.encryption
            .apply(request)
            .send()
            .await
            .map_err(|error| classify("put", &path, error))?;

        Ok(())
    }

    async fn get(&self, at: &ObjectLocation) -> Result<Option<Vec<u8>>, ObjectStoreError> {
        let path = at.as_path();

        let response = self
            .client
            .get_object()
            .bucket(self.bucket.as_str())
            .key(&path)
            .send()
            .await;

        let output = match response {
            Ok(output) => output,
            // Absence is an answer. Every other refusal is not, and telling
            // them apart here is what lets a caller treat "no manifest yet"
            // as a state rather than as a failure to report.
            Err(SdkError::ServiceError(inner)) if inner.err().is_no_such_key() => return Ok(None),
            Err(error) => return Err(classify("get", &path, error)),
        };

        let bytes = output
            .body
            .collect()
            .await
            .map_err(|error| ObjectStoreError::Unavailable {
                operation: "get".to_string(),
                location: path.clone(),
                reason: error.to_string(),
            })?;

        Ok(Some(bytes.to_vec()))
    }

    async fn list(&self, prefix: &ArchivePrefix) -> Result<Vec<ObjectLocation>, ObjectStoreError> {
        let path = prefix.as_path();
        let mut locations = Vec::new();
        let mut pages = self
            .client
            .list_objects_v2()
            .bucket(self.bucket.as_str())
            .prefix(&path)
            .into_paginator()
            .send();

        while let Some(page) = pages.next().await {
            let page = page.map_err(|error| classify("list", &path, error))?;

            for object in page.contents() {
                let Some(key) = object.key() else {
                    continue;
                };

                // Rebuilt through the prefix rather than carried as a string,
                // so a store that returns something outside the prefix it was
                // asked for cannot smuggle it back into a location.
                let Some(relative) = key.strip_prefix(&path) else {
                    warn!(key, prefix = %path, "the store returned a key outside the prefix it was asked for");
                    continue;
                };

                match prefix.object(relative) {
                    Ok(location) => locations.push(location),
                    Err(error) => {
                        warn!(key, %error, "the store returned a key this platform cannot address")
                    }
                }
            }
        }

        Ok(locations)
    }

    async fn delete(&self, at: &ObjectLocation) -> Result<(), ObjectStoreError> {
        let path = at.as_path();

        self.client
            .delete_object()
            .bucket(self.bucket.as_str())
            .key(&path)
            .send()
            .await
            .map_err(|error| classify("delete", &path, error))?;

        Ok(())
    }
}

impl BackupStoreAdmin for S3ObjectStore {
    async fn ensure_bucket(&self, bucket: &BucketName) -> Result<(), ObjectStoreError> {
        let name = bucket.as_str();

        match self.client.head_bucket().bucket(name).send().await {
            Ok(_) => info!(bucket = name, "the archive bucket is there"),
            Err(_) => {
                // Racing another instance starting at the same moment is
                // normal and not worth failing over: whoever lost the race
                // gets an already-owned error and the bucket it wanted.
                match self.client.create_bucket().bucket(name).send().await {
                    Ok(_) => info!(bucket = name, "created the archive bucket"),
                    Err(error) => {
                        if self.client.head_bucket().bucket(name).send().await.is_err() {
                            return Err(classify("create_bucket", name, error));
                        }
                        info!(
                            bucket = name,
                            "the archive bucket appeared while we were creating it"
                        );
                    }
                }
            }
        }

        self.apply_incomplete_upload_rule(bucket).await;
        Ok(())
    }
}

impl S3ObjectStore {
    /// Discards the parts of uploads that never completed.
    ///
    /// Failing to apply this does not stop the platform from starting. A local
    /// store that cannot express a lifecycle rule is a development
    /// inconvenience, while refusing to boot over it would make the whole
    /// system depend on the least capable store anybody points it at. It is
    /// loud instead, because on a real bucket the cost of a missing rule is a
    /// bill nobody can explain.
    async fn apply_incomplete_upload_rule(&self, bucket: &BucketName) {
        let rule = LifecycleRule::builder()
            .id("abort-incomplete-multipart-uploads")
            .status(ExpirationStatus::Enabled)
            .filter(LifecycleRuleFilter::builder().prefix("").build())
            .abort_incomplete_multipart_upload(
                AbortIncompleteMultipartUpload::builder()
                    .days_after_initiation(INCOMPLETE_UPLOAD_GRACE_DAYS as i32)
                    .build(),
            )
            .build();

        let rule = match rule {
            Ok(rule) => rule,
            Err(error) => {
                warn!(%error, "could not build the incomplete upload rule");
                return;
            }
        };

        let configuration = BucketLifecycleConfiguration::builder().rules(rule).build();
        let configuration = match configuration {
            Ok(configuration) => configuration,
            Err(error) => {
                warn!(%error, "could not build the lifecycle configuration");
                return;
            }
        };

        match self
            .client
            .put_bucket_lifecycle_configuration()
            .bucket(bucket.as_str())
            .lifecycle_configuration(configuration)
            .send()
            .await
        {
            Ok(_) => info!(
                bucket = bucket.as_str(),
                days = INCOMPLETE_UPLOAD_GRACE_DAYS,
                "incomplete uploads are discarded by the store"
            ),
            Err(error) => warn!(
                bucket = bucket.as_str(),
                %error,
                "this store did not accept the incomplete upload rule: parts of interrupted uploads will accumulate and be billed, and nothing will list them"
            ),
        }
    }
}

/// How long to wait for the store to accept a connection.
///
/// Short, because a store that has not answered in this long is not busy, it
/// is somewhere else.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// How long a single attempt at one operation may take.
///
/// Generous next to the connect timeout: this covers a transfer, and the only
/// thing it is here to stop is an attempt that hangs for ever.
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(60);

/// How long to wait for a store that is starting up alongside us.
///
/// Compose brings RustFS and the control plane up together, and the control
/// plane wins that race often enough that a first attempt failing means
/// nothing.
pub const STARTUP_GRACE: Duration = Duration::from_secs(30);
