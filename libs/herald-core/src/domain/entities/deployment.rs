use serde::{Deserialize, Serialize};

use super::dataplane::DataPlaneId;
use super::usage::UsageTarget;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeploymentId(pub String);

impl DeploymentId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for DeploymentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Which IAM product a deployment runs.
///
/// Mirrors the control plane's `DeploymentKind` down to the wire spellings.
/// Herald needs it because usage lives behind a different door in each
/// product, and there is no door that works for both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentKind {
    Ferriskey,
    Keycloak,
}

impl std::fmt::Display for DeploymentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ferriskey => write!(f, "ferriskey"),
            Self::Keycloak => write!(f, "keycloak"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deployment {
    pub id: DeploymentId,
    pub dataplane_id: DataPlaneId,
    pub name: String,

    /// Both optional because both are only needed to *find* the instance, and
    /// a control plane that does not say which product this is, or which
    /// namespace it landed in, leaves Herald with nowhere to look. That is a
    /// deployment with no usage to collect, not a reason to stop claiming its
    /// actions.
    pub kind: Option<DeploymentKind>,
    pub namespace: Option<String>,
}

impl Deployment {
    /// Where to read this deployment's usage, when that is knowable at all.
    pub fn usage_target(&self) -> Option<UsageTarget> {
        Some(UsageTarget {
            deployment_id: self.id.clone(),
            kind: self.kind?,
            namespace: self.namespace.clone()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deployment(kind: Option<DeploymentKind>, namespace: Option<&str>) -> Deployment {
        Deployment {
            id: DeploymentId::new("dep-1"),
            dataplane_id: DataPlaneId::new("dp-1"),
            name: "acme-prod".to_string(),
            kind,
            namespace: namespace.map(str::to_string),
        }
    }

    #[test]
    fn a_deployment_that_says_what_it_runs_and_where_has_a_usage_target() {
        let target = deployment(Some(DeploymentKind::Ferriskey), Some("aether-acme"))
            .usage_target()
            .expect("a target");

        assert_eq!(target.kind, DeploymentKind::Ferriskey);
        assert_eq!(target.namespace, "aether-acme");
    }

    #[test]
    fn a_deployment_with_no_namespace_has_nowhere_to_be_read() {
        assert!(
            deployment(Some(DeploymentKind::Ferriskey), None)
                .usage_target()
                .is_none()
        );
    }

    #[test]
    fn a_deployment_with_no_kind_has_nowhere_to_be_read() {
        assert!(
            deployment(None, Some("aether-acme"))
                .usage_target()
                .is_none()
        );
    }
}
