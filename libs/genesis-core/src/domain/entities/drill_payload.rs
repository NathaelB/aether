use serde::Deserialize;
use uuid::Uuid;

use crate::domain::error::GenesisError;

/// What a `deployment.drill` event carries (#185).
///
/// `deployment_id` is the deployment being drilled -- the *source* whose
/// archive is restored, and the id every outcome this drill reports is
/// addressed to. It names nothing genesis creates: the throwaway instance is
/// named after the action, precisely so it is never mistaken for the real
/// one.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DrillPayloadV1 {
    pub deployment_id: Uuid,
    pub organisation_id: Uuid,
    pub kind: String,
    pub version: String,

    /// Sized like the source, not like a default: an archive that does not
    /// fit its own deployment's disk is a fact the drill should notice, not
    /// paper over.
    pub cpu_millis: u32,
    pub memory_mib: u32,
    pub storage_gib: u32,

    /// Where to read the archive from. The same shape `deployment.restore`
    /// carries, and for the same reason: both halves are the source's, and a
    /// data plane rebuilding either would risk restoring somebody else's
    /// data.
    pub source: DrillSourcePayloadV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DrillSourcePayloadV1 {
    pub destination_path: String,
    pub server_name: String,
    #[serde(default)]
    pub backup_id: Option<String>,
}

impl DrillPayloadV1 {
    pub fn from_value(value: &serde_json::Value) -> Result<Self, GenesisError> {
        serde_json::from_value(value.clone()).map_err(|error| GenesisError::InvalidPayload {
            message: format!("invalid drill payload: {error}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn payload() -> serde_json::Value {
        json!({
            "deployment_id": "b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d",
            "organisation_id": "9f8e7d6c-5b4a-3c2d-1e0f-a1b2c3d4e5f6",
            "kind": "ferriskey",
            "version": "26.0.1",
            "cpu_millis": 500,
            "memory_mib": 1024,
            "storage_gib": 5,
            "source": {
                "destination_path": "s3://aether-backups/9f8e7d6c-5b4a-3c2d-1e0f-a1b2c3d4e5f6/b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d",
                "server_name": "deployment-b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d-db",
                "backup_id": "an-archive",
            },
        })
    }

    #[test]
    fn reads_what_it_needs() {
        let parsed = DrillPayloadV1::from_value(&payload()).expect("a valid payload");

        assert_eq!(parsed.kind, "ferriskey");
        assert_eq!(parsed.version, "26.0.1");
        assert_eq!(parsed.storage_gib, 5);
        assert_eq!(
            parsed.source.server_name,
            "deployment-b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d-db"
        );
        assert_eq!(parsed.source.backup_id.as_deref(), Some("an-archive"));
    }

    /// Without a source there is nothing to drill -- a `deployment.create`
    /// payload reaching this handler is a routing mistake, and saying so
    /// beats applying half of it.
    #[test]
    fn refuses_a_payload_with_no_source() {
        let mut value = payload();
        value.as_object_mut().expect("an object").remove("source");

        assert!(DrillPayloadV1::from_value(&value).is_err());
    }
}
