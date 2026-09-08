use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::dataplane::DataPlaneId;
use super::deployment::DeploymentId;
use crate::domain::error::HeraldError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActionId(pub Uuid);

impl std::fmt::Display for ActionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub id: ActionId,
    pub deployment_id: DeploymentId,
    pub dataplane_id: DataPlaneId,
    /// Namespaced action type, e.g. "deployment.create". Equals the control
    /// plane's `action_type` verbatim.
    pub action_type: String,
    pub payload: Value,
    /// Payload schema version. Equals the control plane's `ActionVersion`.
    pub version: u32,
    pub occurred_at: DateTime<Utc>,
}

/// The outcome a caller reports for a single previously-claimed action when
/// acknowledging it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionFailureReason {
    InvalidPayload,
    UnsupportedAction,
    PublishFailed,
    Timeout,
    InternalError(String),
}

/// A single failed action reported when acknowledging outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckFailure {
    pub action_id: ActionId,
    pub reason: ActionFailureReason,
}

/// Result of an `actions:ack` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AckOutcome {
    pub acknowledged: usize,
}

/// Wire format published to the message bus. This is the contract with
/// Genesis: field names and types must not change without updating the
/// consumer side.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionEvent {
    /// Idempotency key. Consumers deduplicate on this.
    pub action_id: Uuid,
    pub deployment_id: Uuid,
    pub dataplane_id: Uuid,
    /// Equals the control plane's `action_type`, e.g. "deployment.create".
    pub routing_key: String,
    /// Equals the control plane's `ActionVersion`. Payload schema version.
    pub version: u32,
    /// Opaque. Not interpreted by Herald.
    pub payload: Value,
    /// Equals the control plane's `metadata.created_at`.
    pub occurred_at: DateTime<Utc>,
}

impl TryFrom<Action> for ActionEvent {
    type Error = HeraldError;

    fn try_from(action: Action) -> Result<Self, Self::Error> {
        let deployment_id = Uuid::parse_str(action.deployment_id.0.trim()).map_err(|err| {
            HeraldError::InvalidAction {
                message: format!(
                    "deployment_id '{}' is not a valid UUID: {err}",
                    action.deployment_id
                ),
            }
        })?;

        let dataplane_id = Uuid::parse_str(action.dataplane_id.0.trim()).map_err(|err| {
            HeraldError::InvalidAction {
                message: format!(
                    "dataplane_id '{}' is not a valid UUID: {err}",
                    action.dataplane_id
                ),
            }
        })?;

        Ok(ActionEvent {
            action_id: action.id.0,
            deployment_id,
            dataplane_id,
            routing_key: action.action_type,
            version: action.version,
            payload: action.payload,
            occurred_at: action.occurred_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_action() -> Action {
        Action {
            id: ActionId(Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap()),
            deployment_id: DeploymentId::new("22222222-2222-2222-2222-222222222222"),
            dataplane_id: DataPlaneId::new("33333333-3333-3333-3333-333333333333"),
            action_type: "deployment.create".to_string(),
            payload: json!({"key": "value"}),
            version: 1,
            occurred_at: DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        }
    }

    #[test]
    fn action_event_serialises_to_the_frozen_wire_shape() {
        let action = sample_action();
        let event = ActionEvent::try_from(action).expect("valid action converts");

        let value = serde_json::to_value(&event).expect("serialisable");

        assert_eq!(
            value.get("action_id").and_then(|v| v.as_str()),
            Some("11111111-1111-1111-1111-111111111111")
        );
        assert_eq!(
            value.get("deployment_id").and_then(|v| v.as_str()),
            Some("22222222-2222-2222-2222-222222222222")
        );
        assert_eq!(
            value.get("dataplane_id").and_then(|v| v.as_str()),
            Some("33333333-3333-3333-3333-333333333333")
        );
        assert_eq!(
            value.get("routing_key").and_then(|v| v.as_str()),
            Some("deployment.create")
        );
        assert_eq!(value.get("version").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(value.get("payload"), Some(&json!({"key": "value"})));
        assert_eq!(
            value.get("occurred_at").and_then(|v| v.as_str()),
            Some("2026-01-01T00:00:00Z")
        );

        // Exactly these seven fields, nothing more, nothing less.
        let map = value.as_object().expect("object");
        let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "action_id",
                "dataplane_id",
                "deployment_id",
                "occurred_at",
                "payload",
                "routing_key",
                "version",
            ]
        );
    }

    #[test]
    fn action_event_rejects_non_uuid_deployment_id() {
        let mut action = sample_action();
        action.deployment_id = DeploymentId::new("not-a-uuid");

        let result = ActionEvent::try_from(action);

        assert!(matches!(result, Err(HeraldError::InvalidAction { .. })));
    }

    #[test]
    fn action_event_rejects_non_uuid_dataplane_id() {
        let mut action = sample_action();
        action.dataplane_id = DataPlaneId::new("not-a-uuid");

        let result = ActionEvent::try_from(action);

        assert!(matches!(result, Err(HeraldError::InvalidAction { .. })));
    }
}
