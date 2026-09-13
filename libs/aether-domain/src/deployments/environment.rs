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

use crate::{CoreError, deployments::DeploymentId};

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

/// How much of the deployment's identifier is appended.
///
/// Eight hexadecimal characters. Long enough that two deployments colliding is
/// not something that happens, short enough that the readable part survives.
///
/// Taken from the **end** of the identifier. A version 7 UUID begins with a
/// timestamp, so two deployments created in the same millisecond share their
/// leading characters -- the randomness is at the other end. Version 4 is
/// random throughout and does not care, and this platform generates both.
const DISCRIMINATOR: usize = 8;

/// Where a deployment's resources live on its cluster.
///
/// Derived here rather than sent by the caller, and derived from the
/// deployment's own identifier rather than from its name alone.
///
/// The name alone was a tenant isolation problem: namespaces were
/// `{environment}-{name}`, nothing constrained them to be unique, and two
/// organisations both calling a deployment `demo` in `development` were given
/// the same namespace. On a shared data plane their two identity providers
/// landed in it together, and everything scoped to a namespace -- network
/// policies, quotas, the object store credentials the operator copies in --
/// stopped separating them.
pub fn namespace_for(environment: Environment, name: &str, deployment: DeploymentId) -> String {
    let identifier = deployment.0.simple().to_string();
    let discriminator = &identifier[identifier.len() - DISCRIMINATOR..];
    let readable = slug(&format!("{environment}-{name}"));

    // The discriminator is what makes this unique, so it is never the part
    // that gets cut. The readable half is trimmed to fit around it.
    let room = MAX_NAMESPACE - DISCRIMINATOR - 1;
    let readable = readable
        .chars()
        .take(room)
        .collect::<String>()
        .trim_end_matches('-')
        .to_string();

    if readable.is_empty() {
        return format!("{environment}-{discriminator}");
    }

    format!("{readable}-{discriminator}")
}

/// A DNS-1123 label: lowercase alphanumerics and hyphens, no run of hyphens,
/// none at either end.
fn slug(value: &str) -> String {
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
    use uuid::Uuid;

    use super::*;

    fn deployment(n: u128) -> DeploymentId {
        DeploymentId(Uuid::from_u128(n))
    }

    /// Two identifiers that differ only in their leading bytes, which is what
    /// a version 7 UUID looks like when two are minted in the same
    /// millisecond. Taking the discriminator from the front would give them
    /// the same namespace.
    fn same_millisecond() -> (DeploymentId, DeploymentId) {
        (
            DeploymentId(Uuid::from_u128(0x0199_0000_0000_0000_0000_0000_0000_0001)),
            DeploymentId(Uuid::from_u128(0x0199_0000_0000_0000_0000_0000_0000_0002)),
        )
    }

    #[test]
    fn two_deployments_minted_in_the_same_millisecond_are_still_told_apart() {
        let (one, other) = same_millisecond();

        assert_ne!(
            namespace_for(Environment::Production, "api", one),
            namespace_for(Environment::Production, "api", other)
        );
    }

    #[test]
    fn a_namespace_reads_as_the_deployment_it_holds() {
        let namespace = namespace_for(Environment::Production, "acme api", deployment(1));

        assert!(namespace.starts_with("production-acme-api-"));
    }

    /// The isolation bug this function exists for. Two organisations naming a
    /// deployment the same thing used to share a namespace on a shared data
    /// plane, and everything scoped to a namespace stopped separating them.
    #[test]
    fn two_deployments_with_the_same_name_never_share_a_namespace() {
        let one = namespace_for(Environment::Development, "demo", deployment(1));
        let other = namespace_for(Environment::Development, "demo", deployment(2));

        assert_ne!(one, other);
    }

    #[test]
    fn a_namespace_is_a_valid_kubernetes_label() {
        for (environment, name) in [
            (Environment::Production, "Ünïcôdé Ñame"),
            (Environment::Staging, "-leading and trailing-"),
            (Environment::Development, "lots???of???separators"),
            (Environment::Production, "a".repeat(200).as_str()),
            (Environment::Staging, ""),
        ] {
            let namespace = namespace_for(environment, name, deployment(7));

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

    /// A name long enough to fill the label must not push the discriminator
    /// out: that is the half that makes it unique.
    #[test]
    fn a_very_long_name_loses_its_tail_rather_than_its_uniqueness() {
        let long = "a".repeat(200);
        let one = namespace_for(Environment::Production, &long, deployment(1));
        let other = namespace_for(Environment::Production, &long, deployment(2));

        assert_ne!(one, other);
        assert!(one.len() <= MAX_NAMESPACE);
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
