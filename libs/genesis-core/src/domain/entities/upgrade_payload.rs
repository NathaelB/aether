use serde::Deserialize;
use uuid::Uuid;

/// What a `deployment.upgrade` event carries.
///
/// Kept apart from `DeploymentPayloadV1` rather than folded into it with more
/// optional fields. The two describe different things: one is a deployment,
/// the other is a step between two versions of one. Sharing a type would mean
/// every field of each is optional to the other, and nothing would be checked.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct UpgradePayloadV1 {
    pub deployment_id: Uuid,
    pub namespace: String,
    /// The version the deployment runs now. Carried so a report about an
    /// upgrade that already happened can be told apart from one about the
    /// upgrade being asked for.
    pub from_version: String,
    pub to_version: String,
}

impl UpgradePayloadV1 {
    pub fn from_value(
        value: &serde_json::Value,
    ) -> Result<Self, crate::domain::error::GenesisError> {
        serde_json::from_value(value.clone()).map_err(|error| {
            crate::domain::error::GenesisError::InvalidPayload {
                message: format!("not an upgrade payload: {error}"),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn payload() -> serde_json::Value {
        json!({
            "deployment_id": "00000000-0000-0000-0000-000000000001",
            "dataplane_id": "00000000-0000-0000-0000-000000000002",
            "organisation_id": "00000000-0000-0000-0000-000000000003",
            "name": "auth",
            "kind": "ferriskey",
            "namespace": "tenant-a",
            "from_version": "26.0.0",
            "to_version": "26.0.1",
        })
    }

    #[test]
    fn reads_what_it_needs_and_ignores_the_rest() {
        let parsed = UpgradePayloadV1::from_value(&payload()).expect("a valid payload");

        assert_eq!(parsed.namespace, "tenant-a");
        assert_eq!(parsed.from_version, "26.0.0");
        assert_eq!(parsed.to_version, "26.0.1");
    }

    /// An upgrade without a target is not an upgrade. Defaulting it to
    /// something would apply a version nobody asked for.
    #[test]
    fn refuses_a_payload_with_no_target() {
        let mut value = payload();
        value
            .as_object_mut()
            .expect("an object")
            .remove("to_version");

        let error = UpgradePayloadV1::from_value(&value).expect_err("no target");

        assert!(error.to_string().contains("to_version"), "{error}");
    }

    /// A `deployment.create` payload reaching this handler is a routing
    /// mistake, and saying so beats applying half of it.
    #[test]
    fn refuses_a_deployment_payload() {
        let value = json!({
            "deployment_id": "00000000-0000-0000-0000-000000000001",
            "namespace": "tenant-a",
            "version": "26.0.1",
        });

        assert!(UpgradePayloadV1::from_value(&value).is_err());
    }
}
