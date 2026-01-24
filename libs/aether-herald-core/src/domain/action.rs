use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::domain::HeraldError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActionId(pub Uuid);

impl ActionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ActionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ActionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionCursor(pub String);

impl ActionCursor {
    pub fn new(cursor: impl Into<String>) -> Self {
        Self(cursor.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionKind {
    Create,
    Update,
    Delete,
    Upsert,
    Custom(String),
}

impl ActionKind {
    pub fn parse(raw: &str) -> Result<Self, HeraldError> {
        let normalized = raw.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return Err(HeraldError::InvalidAction {
                message: "action kind is empty".to_string(),
            });
        }

        let kind = match normalized.as_str() {
            "create" | "created" => ActionKind::Create,
            "update" | "updated" => ActionKind::Update,
            "delete" | "deleted" => ActionKind::Delete,
            "upsert" | "upserted" => ActionKind::Upsert,
            _ => ActionKind::Custom(normalized),
        };

        Ok(kind)
    }

    pub fn as_str(&self) -> &str {
        match self {
            ActionKind::Create => "create",
            ActionKind::Update => "update",
            ActionKind::Delete => "delete",
            ActionKind::Upsert => "upsert",
            ActionKind::Custom(value) => value.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlPlaneAction {
    pub id: ActionId,
    pub resource: String,
    pub kind: String,
    pub payload: Value,
    pub occurred_at: DateTime<Utc>,
}

impl ControlPlaneAction {
    pub fn normalize(&self) -> Result<NormalizedAction, HeraldError> {
        let resource = self.resource.trim();
        if resource.is_empty() {
            return Err(HeraldError::InvalidAction {
                message: "resource is empty".to_string(),
            });
        }

        let kind = ActionKind::parse(&self.kind)?;
        let routing_key = RoutingKey::new(resource, kind.as_str());

        Ok(NormalizedAction {
            id: self.id,
            routing_key,
            payload: self.payload.clone(),
            occurred_at: self.occurred_at,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingKey(pub String);

impl RoutingKey {
    pub fn new(resource: &str, action: &str) -> Self {
        Self(format!("{}.{}", resource.trim(), action.trim()))
    }
}

impl std::fmt::Display for RoutingKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedAction {
    pub id: ActionId,
    pub routing_key: RoutingKey,
    pub payload: Value,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionBatch {
    pub actions: Vec<ControlPlaneAction>,
    pub next_cursor: Option<ActionCursor>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    #[test]
    fn parse_action_kind_is_case_insensitive() {
        assert_eq!(ActionKind::parse("Created").unwrap(), ActionKind::Create);
        assert_eq!(ActionKind::parse("UPDATED").unwrap(), ActionKind::Update);
        assert_eq!(ActionKind::parse("deleted").unwrap(), ActionKind::Delete);
        assert_eq!(ActionKind::parse("Upsert").unwrap(), ActionKind::Upsert);
    }

    #[test]
    fn normalize_action_builds_routing_key() {
        let action = ControlPlaneAction {
            id: ActionId::new(),
            resource: "deployment".to_string(),
            kind: "created".to_string(),
            payload: json!({"id": "dep-1"}),
            occurred_at: Utc.with_ymd_and_hms(2024, 1, 2, 3, 4, 5).unwrap(),
        };

        let normalized = action.normalize().unwrap();

        assert_eq!(normalized.routing_key.to_string(), "deployment.create");
        assert_eq!(normalized.payload, json!({"id": "dep-1"}));
    }

    #[test]
    fn normalize_action_rejects_empty_resource() {
        let action = ControlPlaneAction {
            id: ActionId::new(),
            resource: "   ".to_string(),
            kind: "created".to_string(),
            payload: json!({}),
            occurred_at: Utc.with_ymd_and_hms(2024, 1, 2, 3, 4, 5).unwrap(),
        };

        let error = action.normalize().unwrap_err();

        match error {
            HeraldError::InvalidAction { message } => {
                assert_eq!(message, "resource is empty");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn normalize_action_rejects_empty_kind() {
        let action = ControlPlaneAction {
            id: ActionId::new(),
            resource: "deployment".to_string(),
            kind: "  ".to_string(),
            payload: json!({}),
            occurred_at: Utc.with_ymd_and_hms(2024, 1, 2, 3, 4, 5).unwrap(),
        };

        let error = action.normalize().unwrap_err();

        match error {
            HeraldError::InvalidAction { message } => {
                assert_eq!(message, "action kind is empty");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn action_kind_parse_custom_and_as_str() {
        let kind = ActionKind::parse("Reconcile").unwrap();
        assert_eq!(kind, ActionKind::Custom("reconcile".to_string()));
        assert_eq!(kind.as_str(), "reconcile");
    }

    #[test]
    fn routing_key_trims_parts() {
        let key = RoutingKey::new(" deployment ", " created ");
        assert_eq!(key.0, "deployment.created");
        assert_eq!(key.to_string(), "deployment.created");
    }

    #[test]
    fn action_cursor_new_sets_value() {
        let cursor = ActionCursor::new("cursor-1");
        assert_eq!(cursor.0, "cursor-1");
    }

    #[test]
    fn action_id_display_uses_uuid() {
        let id = ActionId::new();
        assert_eq!(id.to_string(), id.0.to_string());
    }
}
