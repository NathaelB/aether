use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Mirrors the `ActionEvent` produced by `aether-herald-core`.
/// The routing key follows the format `<resource>.<kind>` (e.g. `deployment.create`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionEvent {
    pub action_id: Uuid,
    pub routing_key: String,
    pub payload: Value,
    pub timestamp: DateTime<Utc>,
}

impl ActionEvent {
    /// Returns the resource segment of the routing key (e.g. `"deployment"`).
    pub fn resource(&self) -> &str {
        self.routing_key
            .split_once('.')
            .map(|(r, _)| r)
            .unwrap_or(&self.routing_key)
    }

    /// Returns the kind segment of the routing key (e.g. `"create"`).
    pub fn kind(&self) -> &str {
        self.routing_key
            .split_once('.')
            .map(|(_, k)| k)
            .unwrap_or("")
    }
}
