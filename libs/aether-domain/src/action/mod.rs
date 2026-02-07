use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{dataplane::value_objects::DataPlaneId, deployments::DeploymentId};

pub mod commands;
pub mod ports;
pub mod service;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct ActionId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Action {
    /// Global unique identifier (idempotence key)
    pub id: ActionId,

    pub deployment_id: DeploymentId,

    pub dataplane_id: DataPlaneId,

    /// Namespaced action type (ex: "deployment.create")
    pub action_type: ActionType,

    /// Target resource of the action
    pub target: ActionTarget,

    /// Opaque payload, interpreted by executors (forge agents)
    pub payload: ActionPayload,

    /// Action schema version
    pub version: ActionVersion,

    /// Lifecycle state (control-plane owned)
    pub status: ActionStatus,

    /// Audit & scheduling metadata
    pub metadata: ActionMetadata,

    pub leased_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ActionType(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ActionVersion(pub u32);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ActionTarget {
    /// Domain object kind (Deployment, Realm, Database, …)
    pub kind: TargetKind,

    /// Stable identifier of the target object
    pub id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum TargetKind {
    Deployment,
    Realm,
    Database,
    User,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ActionPayload {
    /// Raw JSON payload
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub enum ActionStatus {
    Pending,
    Leased {
        until: DateTime<Utc>,
    },
    Pulled {
        agent_id: String,
        at: DateTime<Utc>,
    },
    Published {
        at: DateTime<Utc>,
    },
    Failed {
        reason: ActionFailureReason,
        at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub enum ActionFailureReason {
    InvalidPayload,
    UnsupportedAction,
    PublishFailed,
    Timeout,
    InternalError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ActionMetadata {
    /// Who requested the action (user, system, api)
    pub source: ActionSource,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Optional execution constraints
    pub constraints: ActionConstraints,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub enum ActionSource {
    User { user_id: Uuid },
    System,
    Api { client_id: String },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ActionConstraints {
    /// Optional deadline for execution
    pub not_after: Option<DateTime<Utc>>,

    /// Priority hint (lower = more important)
    pub priority: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ActionCursor(pub String);

impl ActionCursor {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ActionBatch {
    pub actions: Vec<Action>,
    pub next_cursor: Option<ActionCursor>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn action_constraints_default_is_empty() {
        let constraints = ActionConstraints::default();

        assert!(constraints.not_after.is_none());
        assert!(constraints.priority.is_none());
    }

    #[test]
    fn action_target_holds_custom_kind() {
        let target = ActionTarget {
            kind: TargetKind::Custom("addon".to_string()),
            id: Uuid::new_v4(),
        };

        assert_eq!(target.kind, TargetKind::Custom("addon".to_string()));
    }

    #[test]
    fn action_status_failed_holds_reason() {
        let status = ActionStatus::Failed {
            reason: ActionFailureReason::InternalError("boom".to_string()),
            at: Utc::now(),
        };

        match status {
            ActionStatus::Failed { reason, .. } => {
                assert_eq!(
                    reason,
                    ActionFailureReason::InternalError("boom".to_string())
                );
            }
            _ => panic!("expected failed status"),
        }
    }
}
