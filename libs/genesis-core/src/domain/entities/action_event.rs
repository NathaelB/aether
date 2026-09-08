use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Mirrors the `ActionEvent` produced by `herald-core`.
///
/// The routing key follows the format `<resource>.<kind>` (e.g. `deployment.create`).
/// Delivery is at-least-once: the same `action_id` can be observed more than once and
/// handlers must converge on the same outcome regardless of how many times, or in what
/// order, events for a given resource are delivered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionEvent {
    /// Idempotency key. The same event may be redelivered under the same `action_id`.
    pub action_id: Uuid,

    pub deployment_id: Uuid,

    pub dataplane_id: Uuid,

    pub routing_key: String,

    /// Payload schema version. `payload` is interpreted per `(routing_key, version)`.
    pub version: u32,

    /// Opaque envelope content, interpreted per `(routing_key, version)`.
    pub payload: Value,

    pub occurred_at: DateTime<Utc>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Deserialises from a literal JSON document shaped exactly like the wire contract
    /// (C1) — not a round-trip of `ActionEvent` itself, so a drift between Herald and
    /// Genesis on field names/types would fail this test.
    #[test]
    fn deserializes_from_the_wire_contract_shape() {
        let raw = json!({
            "action_id": "5b1e6e0a-9a2b-4b8e-8f0a-1a2b3c4d5e6f",
            "deployment_id": "b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d",
            "dataplane_id": "1c2b3a4d-5e6f-4a7b-8c9d-0e1f2a3b4c5e",
            "routing_key": "deployment.create",
            "version": 1,
            "payload": {
                "deployment_id": "b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d",
                "dataplane_id": "1c2b3a4d-5e6f-4a7b-8c9d-0e1f2a3b4c5e",
                "organisation_id": "9f8e7d6c-5b4a-3c2d-1e0f-a1b2c3d4e5f6",
                "name": "acme-prod",
                "kind": "keycloak",
                "version": "25.0.0",
                "namespace": "aether-acme-prod",
                "created_by": "11111111-2222-3333-4444-555555555555"
            },
            "occurred_at": "2026-01-15T10:30:00Z"
        });

        let event: ActionEvent = serde_json::from_value(raw).expect("valid ActionEvent");

        assert_eq!(event.routing_key, "deployment.create");
        assert_eq!(event.version, 1);
        assert_eq!(event.resource(), "deployment");
        assert_eq!(event.kind(), "create");
        assert_eq!(event.payload["name"], "acme-prod");
    }

    #[test]
    fn splits_resource_and_kind() {
        let event = ActionEvent {
            action_id: Uuid::nil(),
            deployment_id: Uuid::nil(),
            dataplane_id: Uuid::nil(),
            routing_key: "deployment.update".to_string(),
            version: 1,
            payload: Value::Null,
            occurred_at: Utc::now(),
        };

        assert_eq!(event.resource(), "deployment");
        assert_eq!(event.kind(), "update");
    }
}
