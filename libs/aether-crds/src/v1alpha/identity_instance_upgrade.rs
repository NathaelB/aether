use std::fmt::Display;

use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::common::types::{Condition, Phase};

#[derive(CustomResource, Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "aether.dev",
    version = "v1alpha",
    kind = "IdentityInstanceUpgrade",
    plural = "identityinstanceupgrades",
    shortname = "iiu",
    namespaced,
    status = "IdentityInstanceUpgradeStatus",
    printcolumn = r#"{"name":"Instance", "type":"string", "jsonPath":".spec.identityInstanceRef.name"}"#,
    printcolumn = r#"{"name":"Target", "type":"string", "jsonPath":".spec.targetVersion"}"#,
    printcolumn = r#"{"name":"Phase", "type":"string", "jsonPath":".status.phase"}"#,
    printcolumn = r#"{"name":"Completed", "type":"boolean", "jsonPath":".status.completed"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct IdentityInstanceUpgradeSpec {
    pub identity_instance_ref: IdentityInstanceRef,

    pub target_version: String,

    #[serde(default)]
    pub strategy: UpgradeStrategy,

    #[serde(default)]
    pub approved: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdentityInstanceRef {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub enum UpgradeStrategy {
    #[default]
    Rolling,
}

impl Display for UpgradeStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rolling => write!(f, "rolling"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct IdentityInstanceUpgradeStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<Phase>,

    #[serde(default)]
    pub completed: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_version: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_version: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<Time>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<Time>,

    /// Whether a completed upgrade is waiting for the controller to delete this resource.
    /// Kept separate from `completed_at` so that field only ever holds a timestamp, never
    /// doubles as a state-machine flag.
    #[serde(default)]
    pub pending_cleanup: bool,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
    use k8s_openapi::chrono::{DateTime, Utc};
    use serde_json::json;

    use crate::common::types::Phase;
    use crate::v1alpha::identity_instance_upgrade::{
        IdentityInstanceRef, IdentityInstanceUpgrade, IdentityInstanceUpgradeSpec,
        IdentityInstanceUpgradeStatus, UpgradeStrategy,
    };
    use kube::core::ObjectMeta;

    fn time(value: &str) -> Time {
        Time(
            DateTime::parse_from_rfc3339(value)
                .unwrap()
                .with_timezone(&Utc),
        )
    }

    #[test]
    fn test_upgrade_strategy_display() {
        assert_eq!(UpgradeStrategy::Rolling.to_string(), "rolling");
    }

    #[test]
    fn test_identity_instance_upgrade_creation() {
        let spec = IdentityInstanceUpgradeSpec {
            identity_instance_ref: IdentityInstanceRef {
                name: "keycloak-example".to_string(),
            },
            target_version: "26.0.0".to_string(),
            strategy: UpgradeStrategy::Rolling,
            approved: true,
        };

        assert_eq!(spec.identity_instance_ref.name, "keycloak-example");
        assert_eq!(spec.target_version, "26.0.0");
        assert!(spec.approved);
    }

    #[test]
    fn test_status_serialization_skips_empty_fields() {
        let status = IdentityInstanceUpgradeStatus::default();
        let value = serde_json::to_value(status).unwrap();

        assert!(value.get("phase").is_none());
        assert_eq!(value["completed"], json!(false));
        assert!(value.get("currentVersion").is_none());
        assert!(value.get("targetVersion").is_none());
        assert!(value.get("startedAt").is_none());
        assert!(value.get("completedAt").is_none());
        assert_eq!(value["pendingCleanup"], json!(false));
        assert!(value.get("conditions").is_none());
        assert!(value.get("message").is_none());
        assert!(value.get("error").is_none());
    }

    #[test]
    fn test_status_helpers_with_values() {
        let resource = IdentityInstanceUpgrade {
            metadata: ObjectMeta {
                namespace: Some("test-aether".to_string()),
                ..Default::default()
            },
            spec: IdentityInstanceUpgradeSpec {
                identity_instance_ref: IdentityInstanceRef {
                    name: "keycloak-example".to_string(),
                },
                target_version: "26.0.0".to_string(),
                strategy: UpgradeStrategy::Rolling,
                approved: true,
            },
            status: Some(IdentityInstanceUpgradeStatus {
                phase: Some(Phase::Updating),
                completed: false,
                current_version: Some("25.0.0".to_string()),
                target_version: Some("26.0.0".to_string()),
                started_at: Some(time("2026-02-10T10:00:00Z")),
                completed_at: None,
                pending_cleanup: false,
                conditions: vec![],
                message: Some("Upgrade in progress".to_string()),
                error: None,
            }),
        };

        assert_eq!(
            resource.spec.identity_instance_ref.name,
            "keycloak-example".to_string()
        );
        assert_eq!(resource.status.unwrap().phase, Some(Phase::Updating));
    }

    #[test]
    fn started_at_and_completed_at_serialize_as_rfc3339_instants_not_flags() {
        let status = IdentityInstanceUpgradeStatus {
            started_at: Some(time("2026-02-10T10:00:00Z")),
            completed_at: Some(time("2026-02-10T10:04:30Z")),
            pending_cleanup: true,
            ..Default::default()
        };

        let value = serde_json::to_value(&status).unwrap();

        assert_eq!(value["startedAt"], json!("2026-02-10T10:00:00Z"));
        assert_eq!(value["completedAt"], json!("2026-02-10T10:04:30Z"));
        assert_eq!(value["pendingCleanup"], json!(true));
    }

    #[test]
    fn an_upgrades_duration_can_be_worked_out_from_the_cr_alone() {
        let status = IdentityInstanceUpgradeStatus {
            started_at: Some(time("2026-02-10T10:00:00Z")),
            completed_at: Some(time("2026-02-10T10:04:30Z")),
            ..Default::default()
        };

        let duration = status.completed_at.unwrap().0 - status.started_at.unwrap().0;

        assert_eq!(duration.num_seconds(), 270);
    }
}
