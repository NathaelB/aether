use std::fmt::Display;

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::common::types::Condition;

const DEFAULT_MAX_UNAVAILABLE: u32 = 1;

#[derive(CustomResource, Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "aether.dev",
    version = "v1alpha",
    kind = "IdentityDataplaneUpgrade",
    plural = "identitydataplaneupgrades",
    shortname = "idpu",
    namespaced,
    status = "IdentityDataplaneUpgradeStatus",
    printcolumn = r#"{"name":"Dataplane", "type":"string", "jsonPath":".spec.dataplaneId"}"#,
    printcolumn = r#"{"name":"Target", "type":"string", "jsonPath":".spec.targetVersion"}"#,
    printcolumn = r#"{"name":"Phase", "type":"string", "jsonPath":".status.phase"}"#,
    printcolumn = r#"{"name":"Progress", "type":"string", "jsonPath":".status.progress"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct IdentityDataplaneUpgradeSpec {
    pub dataplane_id: String,

    pub target_version: String,

    pub components: Vec<DataplaneComponent>,

    #[serde(default)]
    pub strategy: DataplaneUpgradeStrategy,

    /// How many pods of a component may be unavailable at once during the upgrade.
    #[serde(default = "default_max_unavailable")]
    pub max_unavailable: u32,
}

fn default_max_unavailable() -> u32 {
    DEFAULT_MAX_UNAVAILABLE
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub enum DataplaneComponent {
    Herald,
    Genesis,
    Operator,
    All,
}

impl Display for DataplaneComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Herald => write!(f, "Herald"),
            Self::Genesis => write!(f, "Genesis"),
            Self::Operator => write!(f, "Operator"),
            Self::All => write!(f, "All"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub enum DataplaneUpgradeStrategy {
    #[default]
    Rolling,
    Canary,
}

impl Display for DataplaneUpgradeStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rolling => write!(f, "rolling"),
            Self::Canary => write!(f, "canary"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "PascalCase")]
pub enum DataplaneUpgradePhase {
    #[default]
    Pending,
    Upgrading,
    Completed,
    Failed,
    RolledBack,
}

impl Display for DataplaneUpgradePhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Upgrading => write!(f, "Upgrading"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
            Self::RolledBack => write!(f, "RolledBack"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct IdentityDataplaneUpgradeStatus {
    #[serde(default)]
    pub phase: DataplaneUpgradePhase,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_version: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<ComponentUpgradeStatus>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ComponentUpgradeStatus {
    pub name: DataplaneComponent,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_version: Option<String>,

    #[serde(default)]
    pub state: ComponentUpgradeState,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "PascalCase")]
pub enum ComponentUpgradeState {
    #[default]
    Pending,
    Upgrading,
    Upgraded,
    RolledBack,
}

#[cfg(test)]
mod tests {
    use kube::CustomResourceExt;
    use serde_json::json;

    use crate::common::types::{Condition, ConditionStatus};
    use crate::v1alpha::identity_dataplane_upgrade::{
        ComponentUpgradeState, ComponentUpgradeStatus, DataplaneComponent, DataplaneUpgradePhase,
        DataplaneUpgradeStrategy, IdentityDataplaneUpgrade, IdentityDataplaneUpgradeSpec,
        IdentityDataplaneUpgradeStatus,
    };

    #[test]
    fn spec_round_trips_with_camel_case_keys() {
        let spec = IdentityDataplaneUpgradeSpec {
            dataplane_id: "dp-eu-1".to_string(),
            target_version: "0.5.0".to_string(),
            components: vec![DataplaneComponent::Herald, DataplaneComponent::Genesis],
            strategy: DataplaneUpgradeStrategy::Canary,
            max_unavailable: 2,
        };

        let value = serde_json::to_value(&spec).unwrap();

        assert_eq!(
            value,
            json!({
                "dataplaneId": "dp-eu-1",
                "targetVersion": "0.5.0",
                "components": ["Herald", "Genesis"],
                "strategy": "canary",
                "maxUnavailable": 2,
            })
        );
        let back: IdentityDataplaneUpgradeSpec = serde_json::from_value(value).unwrap();
        assert_eq!(back, spec);
    }

    #[test]
    fn spec_defaults_strategy_and_max_unavailable() {
        let spec: IdentityDataplaneUpgradeSpec = serde_json::from_value(json!({
            "dataplaneId": "dp-eu-1",
            "targetVersion": "0.5.0",
            "components": ["All"],
        }))
        .unwrap();

        assert_eq!(spec.strategy, DataplaneUpgradeStrategy::Rolling);
        assert_eq!(spec.max_unavailable, 1);
        assert_eq!(spec.components, vec![DataplaneComponent::All]);
    }

    #[test]
    fn status_defaults_to_pending_and_skips_empty_fields() {
        let status: IdentityDataplaneUpgradeStatus = serde_json::from_value(json!({})).unwrap();

        assert_eq!(status.phase, DataplaneUpgradePhase::Pending);

        let value = serde_json::to_value(&status).unwrap();
        assert_eq!(value, json!({ "phase": "Pending" }));
    }

    #[test]
    fn status_round_trips_with_conditions() {
        let status = IdentityDataplaneUpgradeStatus {
            phase: DataplaneUpgradePhase::RolledBack,
            current_version: Some("0.4.0".to_string()),
            progress: Some("2/3 pods updated".to_string()),
            conditions: vec![Condition {
                condition_type: "Ready".to_string(),
                status: ConditionStatus::False,
                last_transition_time: "2026-02-10T10:00:00Z".to_string(),
                reason: Some("ProbeFailed".to_string()),
                message: Some("readiness probe failed".to_string()),
            }],
            components: Vec::new(),
        };

        let value = serde_json::to_value(&status).unwrap();

        assert_eq!(value["phase"], json!("RolledBack"));
        assert_eq!(value["currentVersion"], json!("0.4.0"));
        assert_eq!(value["progress"], json!("2/3 pods updated"));
        assert_eq!(value["conditions"][0]["type"], json!("Ready"));
        let back: IdentityDataplaneUpgradeStatus = serde_json::from_value(value).unwrap();
        assert_eq!(back, status);
    }

    #[test]
    fn component_records_round_trip_with_camel_case_keys() {
        let status = IdentityDataplaneUpgradeStatus {
            phase: DataplaneUpgradePhase::Upgrading,
            components: vec![ComponentUpgradeStatus {
                name: DataplaneComponent::Herald,
                previous_version: Some("0.4.0".to_string()),
                state: ComponentUpgradeState::Upgrading,
                started_at: Some("2026-02-10T10:00:00Z".to_string()),
            }],
            ..Default::default()
        };

        let value = serde_json::to_value(&status).unwrap();

        assert_eq!(
            value["components"],
            json!([{
                "name": "Herald",
                "previousVersion": "0.4.0",
                "state": "Upgrading",
                "startedAt": "2026-02-10T10:00:00Z",
            }])
        );
        let back: IdentityDataplaneUpgradeStatus = serde_json::from_value(value).unwrap();
        assert_eq!(back, status);
    }

    #[test]
    fn status_written_before_component_records_still_deserializes() {
        let status: IdentityDataplaneUpgradeStatus = serde_json::from_value(json!({
            "phase": "Completed",
            "currentVersion": "0.5.0",
            "progress": "3/3 pods updated",
        }))
        .unwrap();

        assert!(status.components.is_empty());
        assert_eq!(status.phase, DataplaneUpgradePhase::Completed);
    }

    #[test]
    fn displays_match_wire_names() {
        assert_eq!(DataplaneComponent::Operator.to_string(), "Operator");
        assert_eq!(DataplaneUpgradeStrategy::Canary.to_string(), "canary");
        assert_eq!(DataplaneUpgradePhase::RolledBack.to_string(), "RolledBack");
    }

    #[test]
    fn crd_schema_exposes_spec_and_status_properties() {
        let crd = serde_json::to_value(IdentityDataplaneUpgrade::crd()).unwrap();

        assert_eq!(crd["spec"]["group"], json!("aether.dev"));
        assert_eq!(
            crd["spec"]["names"]["kind"],
            json!("IdentityDataplaneUpgrade")
        );
        assert_eq!(crd["spec"]["names"]["shortNames"], json!(["idpu"]));
        assert_eq!(crd["spec"]["scope"], json!("Namespaced"));

        let schema = &crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"];
        let spec = &schema["spec"];
        for key in [
            "dataplaneId",
            "targetVersion",
            "components",
            "strategy",
            "maxUnavailable",
        ] {
            assert!(spec["properties"].get(key).is_some(), "spec.{key} missing");
        }
        assert_eq!(spec["properties"]["maxUnavailable"]["default"], json!(1));
        assert_eq!(spec["properties"]["strategy"]["default"], json!("rolling"));
        assert_eq!(
            spec["required"],
            json!(["components", "dataplaneId", "targetVersion"])
        );

        let status = &schema["status"];
        for key in [
            "phase",
            "currentVersion",
            "progress",
            "conditions",
            "components",
        ] {
            assert!(
                status["properties"].get(key).is_some(),
                "status.{key} missing"
            );
        }
    }
}
