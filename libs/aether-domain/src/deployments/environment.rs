//! Which environment a deployment belongs to, and where it runs.
//!
//! The environment is the one thing in the create form the customer was
//! genuinely answering. It is now stored rather than parsed back out of the
//! namespace, which is what the console had to do: `{environment}-{name}`
//! split on the first hyphen, with no way to tell a deployment called
//! `api-gateway` from an environment nobody named.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::CoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    Production,
    Staging,
    Development,
}

impl Environment {
    pub const ALL: [Self; 3] = [Self::Production, Self::Staging, Self::Development];
}

impl fmt::Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Production => write!(f, "production"),
            Self::Staging => write!(f, "staging"),
            Self::Development => write!(f, "development"),
        }
    }
}

impl FromStr for Environment {
    type Err = CoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_lowercase().as_str() {
            "production" => Ok(Self::Production),
            "staging" => Ok(Self::Staging),
            "development" => Ok(Self::Development),
            other => Err(CoreError::InvalidEnvironment {
                value: other.to_string(),
            }),
        }
    }
}

/// The longest a Kubernetes namespace may be.
const MAX_NAMESPACE: usize = 63;

/// Where a deployment's resources live on its cluster.
///
/// Derived here rather than sent by the caller, and derived from the
/// deployment's organisation rather than from its name alone.
///
/// The name alone was a tenant isolation problem: namespaces were
/// `{environment}-{name}`, nothing constrained them to be unique, and two
/// organisations both calling a deployment `demo` in `development` were given
/// the same namespace. On a shared data plane their two identity providers
/// landed in it together, and everything scoped to a namespace -- network
/// policies, quotas, the object store credentials the operator copies in --
/// stopped separating them.
///
/// An organisation's slug is unique by construction, and a live deployment's
/// name is unique within its own organisation (the same partial index that
/// backs hostname uniqueness), so `{organisation_slug}-{name}` cannot collide
/// with another live deployment's namespace without a UUID discriminator
/// standing in for it.
pub fn namespace_for(organisation_slug: &str, name: &str) -> String {
    let readable = slug(&format!("{organisation_slug}-{name}"));

    readable
        .chars()
        .take(MAX_NAMESPACE)
        .collect::<String>()
        .trim_end_matches('-')
        .to_string()
}

/// A DNS-1123 label: lowercase alphanumerics and hyphens, no run of hyphens,
/// none at either end.
///
/// `pub(crate)`: [`crate::dns::hostname_for`] reuses it rather than deriving
/// its own label from a deployment's name, the same way [`namespace_for`]
/// does for a namespace.
pub(crate) fn slug(value: &str) -> String {
    let mut out = String::with_capacity(value.len());

    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            out.extend(character.to_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }

    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_namespace_reads_as_the_organisation_and_the_deployment_it_holds() {
        let namespace = namespace_for("acme", "acme api");

        assert_eq!(namespace, "acme-acme-api");
    }

    /// The isolation bug this function exists for. Two organisations naming a
    /// deployment the same thing used to share a namespace on a shared data
    /// plane, and everything scoped to a namespace stopped separating them.
    /// Their slugs differ, so the namespace they are given differs too.
    #[test]
    fn two_organisations_naming_a_deployment_alike_never_share_a_namespace() {
        let one = namespace_for("acme", "demo");
        let other = namespace_for("globex", "demo");

        assert_ne!(one, other);
    }

    #[test]
    fn a_namespace_is_a_valid_kubernetes_label() {
        for (organisation_slug, name) in [
            ("acme", "Ünïcôdé Ñame"),
            ("acme", "-leading and trailing-"),
            ("acme", "lots???of???separators"),
            ("acme", "a".repeat(200).as_str()),
            ("acme", ""),
        ] {
            let namespace = namespace_for(organisation_slug, name);

            assert!(namespace.len() <= MAX_NAMESPACE, "too long: {namespace}");
            assert!(!namespace.starts_with('-'), "leading hyphen: {namespace}");
            assert!(!namespace.ends_with('-'), "trailing hyphen: {namespace}");
            assert!(!namespace.contains("--"), "double hyphen: {namespace}");
            assert!(
                namespace
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "not a label: {namespace}"
            );
        }
    }

    /// A name long enough to fill the label truncates rather than panics --
    /// there is no discriminator left to protect, so this only has to stay a
    /// valid, bounded Kubernetes label.
    #[test]
    fn a_very_long_name_truncates_rather_than_panics() {
        let long = "a".repeat(200);
        let namespace = namespace_for("acme", &long);

        assert!(namespace.len() <= MAX_NAMESPACE);
        assert!(namespace.starts_with("acme-"));
    }

    #[test]
    fn an_environment_round_trips_through_its_name() {
        for environment in Environment::ALL {
            assert_eq!(
                environment.to_string().parse::<Environment>().unwrap(),
                environment
            );
        }
    }

    #[test]
    fn an_environment_nobody_named_is_refused() {
        assert!("preprod".parse::<Environment>().is_err());
    }
}
