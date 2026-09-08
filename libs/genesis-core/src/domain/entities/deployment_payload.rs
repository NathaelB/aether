use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::domain::error::GenesisError;

/// Decoded `payload` of a `deployment.*` [`ActionEvent`](super::action_event::ActionEvent),
/// schema version 1. Mirrors the keys written by the control plane in
/// `RecordActionCommand` for `deployment.create`/`deployment.update`/`deployment.delete`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DeploymentPayloadV1 {
    pub deployment_id: Uuid,
    pub dataplane_id: Uuid,
    pub organisation_id: Uuid,
    pub name: String,
    pub kind: String,
    pub version: String,
    pub namespace: String,
    pub created_by: Uuid,
}

impl DeploymentPayloadV1 {
    pub fn from_value(value: &Value) -> Result<Self, GenesisError> {
        serde_json::from_value(value.clone()).map_err(|error| GenesisError::InvalidPayload {
            message: format!("invalid deployment payload: {error}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Deserialises from a literal JSON document shaped like the payload the control
    /// plane actually writes (see `aether-core`'s `RecordActionCommand` construction),
    /// not a round-trip of this struct.
    #[test]
    fn deserializes_from_the_control_plane_payload_shape() {
        let raw = json!({
            "deployment_id": "b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d",
            "dataplane_id": "1c2b3a4d-5e6f-4a7b-8c9d-0e1f2a3b4c5e",
            "organisation_id": "9f8e7d6c-5b4a-3c2d-1e0f-a1b2c3d4e5f6",
            "name": "acme-prod",
            "kind": "keycloak",
            "version": "25.0.0",
            "namespace": "aether-acme-prod",
            "created_by": "11111111-2222-3333-4444-555555555555"
        });

        let payload = DeploymentPayloadV1::from_value(&raw).expect("valid payload");

        assert_eq!(payload.name, "acme-prod");
        assert_eq!(payload.kind, "keycloak");
        assert_eq!(payload.version, "25.0.0");
        assert_eq!(payload.namespace, "aether-acme-prod");
    }

    #[test]
    fn rejects_a_payload_missing_required_fields() {
        let raw = json!({ "name": "acme-prod" });

        let result = DeploymentPayloadV1::from_value(&raw);

        assert!(matches!(result, Err(GenesisError::InvalidPayload { .. })));
    }
}
