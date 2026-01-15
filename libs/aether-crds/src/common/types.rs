use std::fmt::Display;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "PascalCase")]
pub enum Phase {
    #[default]
    Pending,
    Deploying,
    Running,
    Updating,
    Upgrading,
    Maintenance,
    Failed,
    Deleting,
    Terminated,
}

impl Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Deploying => write!(f, "Deploying"),
            Self::Running => write!(f, "Running"),
            Self::Updating => write!(f, "Updating"),
            Self::Upgrading => write!(f, "Upgrading"),
            Self::Maintenance => write!(f, "Maintenance"),
            Self::Failed => write!(f, "Failed"),
            Self::Deleting => write!(f, "Deleting"),
            Self::Terminated => write!(f, "Terminated"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    #[serde(rename = "type")]
    pub condition_type: String,

    pub status: ConditionStatus,

    pub last_transition_time: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ConditionStatus {
    True,
    False,
    Unknown,
}

impl Display for ConditionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::True => write!(f, "True"),
            Self::False => write!(f, "False"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// ResourceRequirements specifies resource requests and limits
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResourceRequirements {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<ResourceList>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<ResourceList>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResourceList {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn phase_display_matches_pascal_case() {
        assert_eq!(Phase::Pending.to_string(), "Pending");
        assert_eq!(Phase::Upgrading.to_string(), "Upgrading");
    }

    #[test]
    fn phase_display_covers_all_variants() {
        let cases = [
            (Phase::Pending, "Pending"),
            (Phase::Deploying, "Deploying"),
            (Phase::Running, "Running"),
            (Phase::Updating, "Updating"),
            (Phase::Upgrading, "Upgrading"),
            (Phase::Maintenance, "Maintenance"),
            (Phase::Failed, "Failed"),
            (Phase::Deleting, "Deleting"),
            (Phase::Terminated, "Terminated"),
        ];

        for (phase, expected) in cases {
            assert_eq!(phase.to_string(), expected);
        }
    }

    #[test]
    fn condition_status_display_matches_title_case() {
        assert_eq!(ConditionStatus::True.to_string(), "True");
        assert_eq!(ConditionStatus::False.to_string(), "False");
        assert_eq!(ConditionStatus::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn phase_serializes_as_pascal_case() {
        let value = serde_json::to_value(Phase::Deploying).unwrap();
        assert_eq!(value, json!("Deploying"));
    }

    #[test]
    fn condition_serializes_with_renamed_type_field() {
        let condition = Condition {
            condition_type: "Ready".to_string(),
            status: ConditionStatus::True,
            last_transition_time: "2025-01-01T00:00:00Z".to_string(),
            reason: None,
            message: None,
        };

        let value = serde_json::to_value(condition).unwrap();
        assert_eq!(value["type"], json!("Ready"));
        assert_eq!(value["status"], json!("True"));
    }

    #[test]
    fn condition_serializes_optional_fields_when_present() {
        let condition = Condition {
            condition_type: "Degraded".to_string(),
            status: ConditionStatus::False,
            last_transition_time: "2025-02-01T00:00:00Z".to_string(),
            reason: Some("Timeout".to_string()),
            message: Some("Readiness probe failed".to_string()),
        };

        let value = serde_json::to_value(condition).unwrap();
        assert_eq!(value["reason"], json!("Timeout"));
        assert_eq!(value["message"], json!("Readiness probe failed"));
    }

    #[test]
    fn resource_requirements_skips_empty_fields() {
        let requirements = ResourceRequirements {
            requests: None,
            limits: Some(ResourceList {
                cpu: Some("500m".to_string()),
                memory: None,
            }),
        };

        let value = serde_json::to_value(requirements).unwrap();
        assert!(value.get("requests").is_none());
        assert_eq!(value["limits"]["cpu"], json!("500m"));
        assert!(value["limits"].get("memory").is_none());
    }
}
