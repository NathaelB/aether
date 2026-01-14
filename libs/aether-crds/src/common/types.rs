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
