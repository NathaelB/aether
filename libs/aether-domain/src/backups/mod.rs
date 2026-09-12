//! Where an archive lives, and what may reach it.
//!
//! The rule this module exists to make unbreakable: a component holding one
//! deployment's archives cannot name another deployment's. Object stores have
//! no notion of a tenant, only of a string, and every cross tenant leak in a
//! shared bucket starts with a key somebody concatenated by hand.
//!
//! So a key is never written whole. An [`ArchivePrefix`] is derived from an
//! organisation and a deployment, and the only way to obtain an
//! [`ObjectLocation`] is to ask a prefix for one. Escaping upwards is refused
//! at parse time rather than resolved, because `..` in a key is either a bug
//! or an attack and neither deserves a best effort interpretation.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{deployments::DeploymentId, organisation::OrganisationId};

pub mod keys;
pub mod ports;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ObjectStoreError {
    /// A key that climbs out of its prefix, or one that cannot be addressed at
    /// all. Refused rather than normalised: a caller that meant a sibling
    /// prefix should say so by building that prefix.
    #[error("'{value}' is not a usable object key: {reason}")]
    InvalidKey { value: String, reason: String },

    #[error("bucket name '{value}' is not valid: {reason}")]
    InvalidBucketName { value: String, reason: String },

    /// The store answered, and the answer was no. Distinct from a transport
    /// failure on purpose: one is retried, the other is reported.
    #[error("{operation} on '{location}' was refused: {reason}")]
    Refused {
        operation: String,
        location: String,
        reason: String,
    },

    #[error("{operation} on '{location}' failed: {reason}")]
    Unavailable {
        operation: String,
        location: String,
        reason: String,
    },
}

/// The bucket an installation archives into.
///
/// One bucket per installation, not per tenant. Tenants are separated by
/// prefix and by the credentials handed to each data plane; a bucket per
/// deployment would put the platform in the business of creating buckets on
/// every provision, and object stores charge for that in quota rather than in
/// money.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BucketName(String);

impl BucketName {
    /// Accepts what every S3 implementation accepts, which is less than any
    /// one of them does alone. RustFS, Scaleway and AWS each allow something
    /// the others refuse, and a name that works locally and fails on the first
    /// real bucket is a deployment that fails at the worst moment.
    pub fn new(value: impl Into<String>) -> Result<Self, ObjectStoreError> {
        let value = value.into();

        let invalid = |reason: &str| ObjectStoreError::InvalidBucketName {
            value: value.clone(),
            reason: reason.to_string(),
        };

        if !(3..=63).contains(&value.len()) {
            return Err(invalid("it must be between 3 and 63 characters"));
        }

        if !value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(invalid(
                "it may only hold lowercase letters, digits and hyphens",
            ));
        }

        let edges_are_alphanumeric = |s: &str| {
            s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
                && s.chars().last().is_some_and(|c| c.is_ascii_alphanumeric())
        };
        if !edges_are_alphanumeric(&value) {
            return Err(invalid("it must start and end with a letter or a digit"));
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BucketName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Everything one deployment archives, addressed as a whole.
///
/// Derived, never parsed. There is no constructor taking a string, so no
/// caller can produce a prefix pointing at a deployment it was not handed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArchivePrefix {
    organisation: OrganisationId,
    deployment: DeploymentId,
}

impl ArchivePrefix {
    pub fn new(organisation: OrganisationId, deployment: DeploymentId) -> Self {
        Self {
            organisation,
            deployment,
        }
    }

    pub fn organisation(&self) -> &OrganisationId {
        &self.organisation
    }

    pub fn deployment(&self) -> &DeploymentId {
        &self.deployment
    }

    /// Addresses one object inside this prefix.
    ///
    /// The only way to build an [`ObjectLocation`]. A caller that wants to
    /// reach another deployment's object has to build that deployment's
    /// prefix, which means holding its identifier, which is the check that
    /// would otherwise live in a comment.
    pub fn object(&self, key: impl AsRef<str>) -> Result<ObjectLocation, ObjectStoreError> {
        Ok(ObjectLocation {
            prefix: self.clone(),
            key: ObjectKey::new(key)?,
        })
    }

    /// What this prefix looks like to the store, with its trailing separator.
    pub fn as_path(&self) -> String {
        format!("{}/{}/", self.organisation, self.deployment)
    }
}

impl fmt::Display for ArchivePrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_path())
    }
}

/// A key relative to an [`ArchivePrefix`].
///
/// Relative is the point. An absolute key would be addressable on its own, and
/// then nothing would stop it naming another tenant.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectKey(String);

impl ObjectKey {
    fn new(value: impl AsRef<str>) -> Result<Self, ObjectStoreError> {
        let value = value.as_ref();

        let invalid = |reason: &str| ObjectStoreError::InvalidKey {
            value: value.to_string(),
            reason: reason.to_string(),
        };

        if value.is_empty() {
            return Err(invalid("it is empty"));
        }

        if value.starts_with('/') {
            return Err(invalid("it is absolute, and a key is relative to a prefix"));
        }

        if value.ends_with('/') {
            return Err(invalid("it names a prefix rather than an object"));
        }

        if value.contains('\0') {
            return Err(invalid("it holds a null byte"));
        }

        // The whole reason this type exists. `a/../../b` resolves out of the
        // prefix on any store that normalises, and stays a literal key on any
        // store that does not. Both outcomes are wrong and they differ per
        // implementation, so neither is allowed to happen.
        if value
            .split('/')
            .any(|segment| segment == ".." || segment == ".")
        {
            return Err(invalid("it climbs out of its prefix"));
        }

        if value.split('/').any(str::is_empty) {
            return Err(invalid("it holds an empty path segment"));
        }

        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ObjectKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One object, in one deployment's prefix.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectLocation {
    prefix: ArchivePrefix,
    key: ObjectKey,
}

impl ObjectLocation {
    pub fn prefix(&self) -> &ArchivePrefix {
        &self.prefix
    }

    pub fn key(&self) -> &ObjectKey {
        &self.key
    }

    /// The full path inside the bucket. What the adapter hands to the store,
    /// and the only place the two halves are joined.
    pub fn as_path(&self) -> String {
        format!("{}{}", self.prefix.as_path(), self.key)
    }
}

impl fmt::Display for ObjectLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_path())
    }
}

/// How long an interrupted upload is kept before the store discards it.
///
/// Not a preference. A multipart upload that never completes leaves its parts
/// in the bucket, they appear in no listing, and they are billed until
/// somebody goes looking for them with a tool that can see them. Every bucket
/// this platform writes to gets this rule, which is why it is part of
/// provisioning rather than a setting.
pub const INCOMPLETE_UPLOAD_GRACE_DAYS: u32 = 7;

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn prefix() -> ArchivePrefix {
        ArchivePrefix::new(
            OrganisationId(Uuid::from_u128(1)),
            DeploymentId(Uuid::from_u128(2)),
        )
    }

    #[test]
    fn an_object_is_addressed_under_its_own_prefix() {
        let location = prefix().object("base/20260912.tar").unwrap();

        assert_eq!(
            location.as_path(),
            format!(
                "{}/{}/base/20260912.tar",
                Uuid::from_u128(1),
                Uuid::from_u128(2)
            )
        );
    }

    #[test]
    fn a_key_cannot_climb_out_of_its_prefix() {
        for escape in ["../other/secret", "base/../../other", "./base", ".."] {
            assert!(
                prefix().object(escape).is_err(),
                "'{escape}' was accepted and it addresses another prefix"
            );
        }
    }

    #[test]
    fn a_key_is_relative_and_names_an_object() {
        for refused in ["", "/absolute", "trailing/", "double//segment"] {
            assert!(
                prefix().object(refused).is_err(),
                "'{refused}' was accepted"
            );
        }
    }

    #[test]
    fn two_deployments_never_share_a_prefix() {
        let one = ArchivePrefix::new(
            OrganisationId(Uuid::from_u128(1)),
            DeploymentId(Uuid::from_u128(2)),
        );
        let other = ArchivePrefix::new(
            OrganisationId(Uuid::from_u128(1)),
            DeploymentId(Uuid::from_u128(3)),
        );

        assert_ne!(one.as_path(), other.as_path());
        assert!(!one.as_path().starts_with(&other.as_path()));
    }

    #[test]
    fn a_bucket_name_every_store_accepts() {
        assert!(BucketName::new("aether-backups").is_ok());

        for refused in ["ab", "Aether", "aether_backups", "-aether", "aether-"] {
            assert!(
                BucketName::new(refused).is_err(),
                "'{refused}' was accepted"
            );
        }
    }
}
