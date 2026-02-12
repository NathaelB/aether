use std::fmt::Display;

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::common::types::{Condition, Phase, ResourceRequirements};

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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ferriskey: Option<FerriskeyConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ingress: Option<IngressConfig>,
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
pub struct FerriskeyConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webapp_url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IngressConfig {
    #[serde(default = "default_ingress_enabled")]
    pub enabled: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls: Option<IngressTlsConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IngressTlsConfig {
    #[serde(default = "default_ingress_tls_enabled")]
    pub enabled: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_issuer: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseConfig {
    #[serde(default)]
    pub mode: DatabaseMode,

    pub managed_cluster: ManagedClusterConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub enum DatabaseMode {
    #[default]
    ManagedCluster,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManagedClusterConfig {
    #[serde(default = "default_instances")]
    pub instances: i32,

    pub storage: ManagedClusterStorage,

    pub resources: ResourceRequirements,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManagedClusterStorage {
    pub size: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_class: Option<String>,
}

fn default_instances() -> i32 {
    1
}

fn default_ingress_enabled() -> bool {
    true
}

fn default_ingress_tls_enabled() -> bool {
    true
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
    use serde_json::json;

    use crate::common::types::{Phase, ResourceList, ResourceRequirements};
    use crate::v1alpha::identity_instance::{
        DatabaseConfig, DatabaseMode, IdentityInstance, IdentityInstanceSpec, IdentityProvider,
        ManagedClusterConfig, ManagedClusterStorage,
    };
    use kube::core::ObjectMeta;

    #[test]
    fn test_identity_instance_creation() {
        let spec = IdentityInstanceSpec {
            organisation_id: "org-123".to_string(),
            provider: IdentityProvider::Keycloak,
            version: "25.0.0".to_string(),
            hostname: "auth.acme.com".to_string(),
            database: DatabaseConfig {
                mode: DatabaseMode::ManagedCluster,
                managed_cluster: ManagedClusterConfig {
                    instances: 2,
                    storage: ManagedClusterStorage {
                        size: "20Gi".to_string(),
                        storage_class: Some("fast-ssd".to_string()),
                    },
                    resources: ResourceRequirements {
                        requests: Some(ResourceList {
                            cpu: Some("500m".to_string()),
                            memory: Some("1Gi".to_string()),
                        }),
                        limits: Some(ResourceList {
                            cpu: Some("2".to_string()),
                            memory: Some("4Gi".to_string()),
                        }),
                    },
                },
            },
            ferriskey: None,
            ingress: None,
        };

        assert_eq!(spec.provider, IdentityProvider::Keycloak);
        assert_eq!(spec.hostname, "auth.acme.com");
        assert_eq!(spec.database.managed_cluster.instances, 2);
    }

    #[test]
    fn test_provider_display() {
        assert_eq!(IdentityProvider::Keycloak.to_string(), "keycloak");
        assert_eq!(IdentityProvider::Ferriskey.to_string(), "ferriskey");
    }

    #[test]
    fn test_default_instances() {
        let config = DatabaseConfig {
            mode: DatabaseMode::ManagedCluster,
            managed_cluster: ManagedClusterConfig {
                instances: super::default_instances(),
                storage: ManagedClusterStorage {
                    size: "10Gi".to_string(),
                    storage_class: None,
                },
                resources: ResourceRequirements {
                    requests: None,
                    limits: None,
                },
            },
        };

        assert_eq!(config.managed_cluster.instances, 1);
    }

    #[test]
    fn test_database_config_deserializes_managed_cluster() {
        let value = json!({
            "mode": "managedCluster",
            "managedCluster": {
                "instances": 3,
                "storage": {
                    "size": "50Gi",
                    "storageClass": "premium-rwo"
                },
                "resources": {
                    "requests": { "cpu": "500m", "memory": "1Gi" },
                    "limits": { "cpu": "2", "memory": "4Gi" }
                }
            }
        });

        let config: DatabaseConfig = serde_json::from_value(value).unwrap();
        assert_eq!(config.mode, DatabaseMode::ManagedCluster);
        assert_eq!(config.managed_cluster.instances, 3);
        assert_eq!(config.managed_cluster.storage.size, "50Gi");
    }

    #[test]
    fn test_identity_instance_status_helpers() {
        let instance = IdentityInstance {
            metadata: ObjectMeta {
                namespace: Some("default".to_string()),
                ..Default::default()
            },
            spec: IdentityInstanceSpec {
                organisation_id: "org-123".to_string(),
                provider: IdentityProvider::Keycloak,
                version: "25.0.0".to_string(),
                hostname: "auth.acme.com".to_string(),
                database: DatabaseConfig {
                    mode: DatabaseMode::ManagedCluster,
                    managed_cluster: ManagedClusterConfig {
                        instances: 1,
                        storage: ManagedClusterStorage {
                            size: "10Gi".to_string(),
                            storage_class: None,
                        },
                        resources: ResourceRequirements {
                            requests: None,
                            limits: None,
                        },
                    },
                },
                ferriskey: None,
                ingress: None,
            },
            status: Some(super::IdentityInstanceStatus {
                phase: Some(Phase::Running),
                ready: true,
                endpoint: Some("https://auth.acme.com".to_string()),
                admin_url: None,
                conditions: vec![],
                last_updated: None,
                error: None,
            }),
        };

        assert!(instance.is_ready());
        assert_eq!(instance.phase(), Some(Phase::Running));
        assert_eq!(
            instance.endpoint(),
            Some("https://auth.acme.com".to_string())
        );
        assert_eq!(instance.namespace(), Some("default".to_string()));
    }

    #[test]
    fn test_identity_instance_helpers_with_missing_status() {
        let instance = IdentityInstance {
            metadata: ObjectMeta::default(),
            spec: IdentityInstanceSpec {
                organisation_id: "org-123".to_string(),
                provider: IdentityProvider::Ferriskey,
                version: "1.0.0".to_string(),
                hostname: "auth.example.com".to_string(),
                database: DatabaseConfig {
                    mode: DatabaseMode::ManagedCluster,
                    managed_cluster: ManagedClusterConfig {
                        instances: 1,
                        storage: ManagedClusterStorage {
                            size: "10Gi".to_string(),
                            storage_class: None,
                        },
                        resources: ResourceRequirements {
                            requests: None,
                            limits: None,
                        },
                    },
                },
                ferriskey: None,
                ingress: None,
            },
            status: None,
        };

        assert!(!instance.is_ready());
        assert_eq!(instance.phase(), None);
        assert_eq!(instance.endpoint(), None);
        assert_eq!(instance.namespace(), None);
    }

    #[test]
    fn test_identity_instance_status_serialization_skips_empty_fields() {
        let status = super::IdentityInstanceStatus::default();
        let value = serde_json::to_value(status).unwrap();

        assert!(value.get("phase").is_none());
        assert_eq!(value["ready"], json!(false));
        assert!(value.get("endpoint").is_none());
        assert!(value.get("adminUrl").is_none());
        assert!(value.get("conditions").is_none());
        assert!(value.get("lastUpdated").is_none());
        assert!(value.get("error").is_none());
    }

    #[test]
    fn test_database_mode_defaults_to_managed_cluster() {
        let value = json!({
            "managedCluster": {
                "instances": 1,
                "storage": { "size": "10Gi" },
                "resources": {}
            }
        });

        let config: DatabaseConfig = serde_json::from_value(value).unwrap();
        assert_eq!(config.mode, DatabaseMode::ManagedCluster);
    }
}
