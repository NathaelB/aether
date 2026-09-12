//! Which key an archive was encrypted under, and how to get it back.
//!
//! Envelope encryption, and the reason for it is arithmetic rather than
//! taste. A key manager encrypts a few kilobytes per call, so it cannot
//! encrypt an archive; it encrypts the key that does. Rotating then costs a
//! rewrap of a few hundred bytes instead of rewriting terabytes, and
//! destroying a key makes every archive under it unreadable without touching
//! the bucket, which is the only deletion story that works when the archives
//! are immutable.
//!
//! The rule this module makes structural: **a rotation never rewrites
//! history**. Every archive records the key version that wrapped it, and a new
//! version is an addition. An installation that rewrapped in place would have
//! no way to state what a given archive was encrypted under, which is the one
//! question an audit asks.

use std::fmt;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub use crate::backups::ObjectStoreError;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum KeyError {
    /// The key manager answered, and the key is not usable: destroyed,
    /// revoked, or never there.
    ///
    /// Distinct from every other failure on purpose, and the distinction is
    /// the whole point of this enum. A restore that fails because the data is
    /// corrupt and a restore that fails because nobody can decrypt it any more
    /// are different incidents with different answers, and an hour spent
    /// telling them apart is an hour spent during an outage.
    #[error("key '{key}' cannot be used: {reason}")]
    KeyUnavailable { key: String, reason: String },

    /// The key manager did not answer at all. Retrying may work; the key is
    /// not known to be gone.
    #[error("the key manager is unreachable: {reason}")]
    ProviderUnavailable { reason: String },

    #[error("{operation} was refused: {reason}")]
    Refused { operation: String, reason: String },

    #[error("the key manager returned something this platform cannot read: {reason}")]
    Malformed { reason: String },

    #[error("'{value}' does not name a key: {reason}")]
    InvalidKeyName { value: String, reason: String },
}

/// Which configured key manager a key lives in.
///
/// A name rather than an enum of the providers this platform happens to
/// support today. Adding a customer's own manager is then a line of
/// configuration and an adapter, not a variant every match in the codebase has
/// to learn about.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[schema(value_type = String, example = "platform")]
pub struct ProviderName(String);

impl ProviderName {
    /// The key this installation holds itself, used when nobody has asked for
    /// anything else.
    pub const PLATFORM: &'static str = "platform";

    pub fn platform() -> Self {
        Self(Self::PLATFORM.to_string())
    }

    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The key inside its manager.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[schema(value_type = String, example = "aether-backups")]
pub struct KeyName(String);

impl KeyName {
    pub fn new(value: impl Into<String>) -> Result<Self, KeyError> {
        let value = value.into();

        let invalid = |reason: &str| KeyError::InvalidKeyName {
            value: value.clone(),
            reason: reason.to_string(),
        };

        if value.is_empty() {
            return Err(invalid("it is empty"));
        }

        // A key name goes into a URL path on every manager this platform
        // talks to. A name holding a separator addresses something else, and
        // finding that out at rotation time is finding it out too late.
        if !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(invalid(
                "it may only hold letters, digits, hyphens and underscores",
            ));
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for KeyName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Which version of a key wrapped one archive.
///
/// Recorded rather than resolved at read time. "The current version" is not an
/// answer to "what encrypted this", and an installation that rotated twice
/// would have no way to reconstruct it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema,
)]
#[schema(value_type = u32, example = 1)]
pub struct KeyVersion(u32);

impl KeyVersion {
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    pub fn value(self) -> u32 {
        self.0
    }
}

impl fmt::Display for KeyVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}", self.0)
    }
}

/// Everything needed to ask for a wrapped key back.
///
/// Carried by every archive, and written beside it as well as into the
/// database. An installation that lost its control plane database still has a
/// bucket full of archives that say what they were encrypted under.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct KeyRef {
    pub provider: ProviderName,
    pub name: KeyName,
    pub version: KeyVersion,
}

impl KeyRef {
    pub fn new(provider: ProviderName, name: KeyName, version: KeyVersion) -> Self {
        Self {
            provider,
            name,
            version,
        }
    }
}

impl fmt::Display for KeyRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}/{}", self.provider, self.name, self.version)
    }
}

/// Key material in the clear.
///
/// No `Debug`, no `Display`, no `Serialize`, and zeroed when it goes out of
/// scope. Every one of those is a way key material reaches a log, and a log is
/// the one place it can never be taken back from.
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct Dek(Vec<u8>);

impl Dek {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn expose(&self) -> &[u8] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A data key as the manager returned it, safe to store beside the archive.
///
/// A string rather than bytes because that is what every manager this platform
/// talks to returns, and what goes into the JSON header written next to an
/// archive. Opaque here: only the manager that produced it can read it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[schema(value_type = String)]
pub struct WrappedDek(String);

impl WrappedDek {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A fresh key, in both the forms the caller needs at once.
///
/// Handed over together because the plaintext is only ever available at
/// generation. Asking for it again means unwrapping, and a caller that dropped
/// it has to go back to the manager, which is the behaviour worth making
/// obvious rather than convenient.
pub struct DataKey {
    plaintext: Dek,
    wrapped: WrappedDek,
    key: KeyRef,
}

impl DataKey {
    pub fn new(plaintext: Dek, wrapped: WrappedDek, key: KeyRef) -> Self {
        Self {
            plaintext,
            wrapped,
            key,
        }
    }

    pub fn plaintext(&self) -> &Dek {
        &self.plaintext
    }

    pub fn wrapped(&self) -> &WrappedDek {
        &self.wrapped
    }

    /// What the archive records: which key, which version.
    pub fn key(&self) -> &KeyRef {
        &self.key
    }

    pub fn into_parts(self) -> (Dek, WrappedDek, KeyRef) {
        (self.plaintext, self.wrapped, self.key)
    }
}

/// Deliberately says nothing. The whole type exists to keep key material out
/// of logs, and a derived `Debug` on the struct holding it would put it back.
impl fmt::Debug for DataKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DataKey").field("key", &self.key).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_name_cannot_address_something_else() {
        assert!(KeyName::new("aether-backups").is_ok());

        for refused in ["", "aether/backups", "../keys/other", "aether backups"] {
            assert!(KeyName::new(refused).is_err(), "'{refused}' was accepted");
        }
    }

    #[test]
    fn a_reference_names_the_version_and_not_just_the_key() {
        let reference = KeyRef::new(
            ProviderName::platform(),
            KeyName::new("aether-backups").unwrap(),
            KeyVersion::new(3),
        );

        assert_eq!(reference.to_string(), "platform/aether-backups/v3");
    }

    #[test]
    fn two_versions_of_one_key_are_two_references() {
        let name = KeyName::new("aether-backups").unwrap();
        let first = KeyRef::new(ProviderName::platform(), name.clone(), KeyVersion::new(1));
        let second = KeyRef::new(ProviderName::platform(), name, KeyVersion::new(2));

        assert_ne!(
            first, second,
            "a rotation that produced an equal reference would rewrite what an archive says it was encrypted under"
        );
    }

    #[test]
    fn a_data_key_does_not_print_its_key_material() {
        let material = vec![7u8; 32];
        let key = DataKey::new(
            Dek::new(material.clone()),
            WrappedDek::new("wrapped:v1:opaque"),
            KeyRef::new(
                ProviderName::platform(),
                KeyName::new("aether-backups").unwrap(),
                KeyVersion::new(1),
            ),
        );

        let printed = format!("{key:?}");

        assert!(
            !printed.contains("117") && !printed.contains("[7"),
            "key material reached a formatter: {printed}"
        );
        assert!(
            printed.contains("aether-backups"),
            "the reference is what makes a log line useful, and it is missing: {printed}"
        );
        assert_eq!(key.plaintext().expose(), material.as_slice());
    }
}
