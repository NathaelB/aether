//! Where a deployment's archives go, and what the store is asked to do with
//! them.
//!
//! The layout belongs to the control plane. A data plane deriving the path
//! itself would be a second implementation of the prefix rule, and the two
//! would disagree the first time either changed -- silently, because an
//! archive written to the wrong prefix is still an archive.

use std::fmt;

use crate::backups::{ArchivePrefix, BucketName};

/// How the object store is asked to encrypt what it is given.
///
/// Worth having and worth not overstating. The store decrypts on read, so this
/// protects the disks under the bucket and does not lock the provider out of
/// it. An archive nobody but the customer can read is a different mechanism,
/// encrypted before it leaves the data plane, and it is a separate chantier.
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

    /// The same instruction in the vocabulary a Postgres operator writes into
    /// its object store section.
    ///
    /// `None` means the bucket's own policy decides, which is what absence
    /// means there. It is not the same as [`StoreEncryption::None`], and the
    /// two reading alike in English is why this returns an option rather than
    /// a string that can be empty.
    pub fn as_archive_directive(&self) -> Option<&'static str> {
        match self {
            Self::Managed => Some("AES256"),
            Self::ProviderKey { .. } => Some("aws:kms"),
            Self::None => None,
        }
    }
}

/// Where one deployment's archives go.
///
/// Built from a bucket and a prefix, never parsed. There is no constructor
/// taking a path, so nothing can be handed a destination pointing at a
/// deployment it was not given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveDestination {
    bucket: BucketName,
    prefix: ArchivePrefix,
}

impl ArchiveDestination {
    pub fn new(bucket: BucketName, prefix: ArchivePrefix) -> Self {
        Self { bucket, prefix }
    }

    pub fn prefix(&self) -> &ArchivePrefix {
        &self.prefix
    }

    /// `s3://bucket/organisation/deployment`, with no trailing slash.
    ///
    /// The trailing slash matters: barman appends its own folders, and a
    /// destination ending in one produces a double separator that some stores
    /// treat as an empty path segment and others as part of the key.
    pub fn as_url(&self) -> String {
        format!(
            "s3://{}/{}",
            self.bucket,
            self.prefix.as_path().trim_end_matches('/')
        )
    }
}

impl fmt::Display for ArchiveDestination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_url())
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::{deployments::DeploymentId, organisation::OrganisationId};

    fn destination() -> ArchiveDestination {
        ArchiveDestination::new(
            BucketName::new("aether-backups").unwrap(),
            ArchivePrefix::new(
                OrganisationId(Uuid::from_u128(1)),
                DeploymentId(Uuid::from_u128(2)),
            ),
        )
    }

    #[test]
    fn a_destination_addresses_one_deployment_under_the_bucket() {
        assert_eq!(
            destination().as_url(),
            format!(
                "s3://aether-backups/{}/{}",
                Uuid::from_u128(1),
                Uuid::from_u128(2)
            )
        );
    }

    #[test]
    fn a_destination_has_no_trailing_separator() {
        // Barman appends its own folders. A destination ending in a separator
        // produces a double one, which stores disagree about.
        assert!(!destination().as_url().ends_with('/'));
    }

    #[test]
    fn two_deployments_never_share_a_destination() {
        let other = ArchiveDestination::new(
            BucketName::new("aether-backups").unwrap(),
            ArchivePrefix::new(
                OrganisationId(Uuid::from_u128(1)),
                DeploymentId(Uuid::from_u128(3)),
            ),
        );

        assert_ne!(destination().as_url(), other.as_url());
    }

    #[test]
    fn a_store_encryption_mode_is_read_from_configuration() {
        assert_eq!(
            StoreEncryption::parse("managed"),
            Ok(StoreEncryption::Managed)
        );
        assert_eq!(StoreEncryption::parse(""), Ok(StoreEncryption::Managed));
        assert_eq!(StoreEncryption::parse("none"), Ok(StoreEncryption::None));
        assert_eq!(
            StoreEncryption::parse("kms:abc-123"),
            Ok(StoreEncryption::ProviderKey {
                key_id: "abc-123".to_string()
            })
        );
        assert!(StoreEncryption::parse("kms:").is_err());
        assert!(StoreEncryption::parse("sometimes").is_err());
    }

    /// Absent is the bucket's own policy, which is not the same answer as
    /// "encrypt nothing" even though the two read alike.
    #[test]
    fn switching_encryption_off_leaves_the_directive_out() {
        assert_eq!(
            StoreEncryption::Managed.as_archive_directive(),
            Some("AES256")
        );
        assert_eq!(
            StoreEncryption::ProviderKey {
                key_id: "k".to_string()
            }
            .as_archive_directive(),
            Some("aws:kms")
        );
        assert_eq!(StoreEncryption::None.as_archive_directive(), None);
    }
}
