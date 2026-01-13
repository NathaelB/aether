use std::fmt::Display;

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::common::types::{Condition, Phase};

#[derive(CustomResource, Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "aether.dev",
    version = "v1alpha",
    kind = "IdentityInstance",
    plural = "identityinstances",
    shortname = "ii",
    namespaced,
    status = "IdentityInstanceStatus",
    printcolumn = r#"{"name":"Provider", "type":"string", "jsonPath":".spec.provider"}"#,
    printcolumn = r#"{"name":"Version", "type":"string", "jsonPath":".spec.version"}"#,
    printcolumn = r#"{"name":"Phase", "type":"string", "jsonPath":".status.phase"}"#,
    printcolumn = r#"{"name":"Ready", "type":"boolean", "jsonPath":".status.ready"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct IdentityInstanceSpec {
    pub organisation_id: String,

    pub provider: IdentityProvider,

    pub version: String,

    pub hostname: String,

    pub database: DatabaseConfig,
}

/// Status of the IdentityInstance
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct IdentityInstanceStatus {
    /// Current phase of the instance
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<Phase>,

    /// Is the instance ready to serve traffic
    #[serde(default)]
    pub ready: bool,

    /// Public endpoint URL (e.g., https://auth.acme.com)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,

    /// Admin console URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_url: Option<String>,

    /// Conditions represent the latest available observations
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,

    /// Last time the status was updated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,

    /// Error message if the instance is in Failed phase
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum IdentityProvider {
    Keycloak,
    Ferriskey,
}

impl Display for IdentityProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Keycloak => write!(f, "keycloak"),
            Self::Ferriskey => write!(f, "ferriskey"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseConfig {
    pub host: String,

    #[serde(default = "default_db_port")]
    pub port: i32,

    pub name: String,

    pub credentials_secret: String,
}

fn default_db_port() -> i32 {
    5432
}

impl IdentityInstance {
    pub fn is_ready(&self) -> bool {
        self.status.as_ref().map(|s| s.ready).unwrap_or(false)
    }

    pub fn phase(&self) -> Option<Phase> {
        self.status.as_ref().and_then(|s| s.phase.clone())
    }

    pub fn endpoint(&self) -> Option<String> {
        self.status.as_ref().and_then(|s| s.endpoint.clone())
    }

    pub fn namespace(&self) -> Option<String> {
        self.metadata.namespace.clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::v1alpha::identity_instance::{
        DatabaseConfig, IdentityInstanceSpec, IdentityProvider, default_db_port,
    };

    #[test]
    fn test_identity_instance_creation() {
        let spec = IdentityInstanceSpec {
            organisation_id: "org-123".to_string(),
            provider: IdentityProvider::Keycloak,
            version: "25.0.0".to_string(),
            hostname: "auth.acme.com".to_string(),
            database: DatabaseConfig {
                host: "postgres.default.svc".to_string(),
                port: 5432,
                name: "keycloak_acme".to_string(),
                credentials_secret: "keycloak-db-creds".to_string(),
            },
        };

        assert_eq!(spec.provider, IdentityProvider::Keycloak);
        assert_eq!(spec.hostname, "auth.acme.com");
    }

    #[test]
    fn test_provider_display() {
        assert_eq!(IdentityProvider::Keycloak.to_string(), "keycloak");
        assert_eq!(IdentityProvider::Ferriskey.to_string(), "ferriskey");
    }

    #[test]
    fn test_default_db_port() {
        let config = DatabaseConfig {
            host: "localhost".to_string(),
            port: default_db_port(),
            name: "test".to_string(),
            credentials_secret: "secret".to_string(),
        };

        assert_eq!(config.port, 5432);
    }
}
