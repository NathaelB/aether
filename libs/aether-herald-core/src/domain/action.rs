use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::deployment::DeploymentId;

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
    pub resource: String,
    pub kind: String,
    pub payload: Value,
    pub occurred_at: DateTime<Utc>,
}

/// TODO: Ensure structure is valid for consumption by agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionEvent {
    pub action_id: ActionId,
    pub routing_key: String,
    pub payload: Value,
    pub timestamp: DateTime<Utc>,
}

impl From<Action> for ActionEvent {
    fn from(action: Action) -> Self {
        let routing_key = format!("{}.{}", action.resource.trim(), action.kind.trim());
        ActionEvent {
            action_id: action.id,
            routing_key: routing_key,
            payload: action.payload,
            timestamp: action.occurred_at,
        }
    }
}
