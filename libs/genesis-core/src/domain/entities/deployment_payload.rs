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
    /// Sizing decided by the control plane when it placed this deployment.
    ///
    /// Optional so an action recorded before the control plane carried it can
    /// still be consumed -- at-least-once delivery means the queue can hold
    /// events older than this field. Absent, the defaults below apply, which
    /// is what genesis invented for every deployment until now.
    #[serde(default)]
    pub cpu_millis: Option<u32>,
    #[serde(default)]
    pub memory_mib: Option<u32>,
    #[serde(default)]
    pub storage_gib: Option<u32>,

    /// Where this deployment archives, and when.
    ///
    /// Optional, and absence means the installation archives nowhere. It is
    /// also what an action recorded before the control plane sent this looks
    /// like -- at-least-once delivery means the queue can hold events older
    /// than the field.
    #[serde(default)]
    pub archive: Option<ArchivePayloadV1>,

    /// Where this deployment's database comes from, when it is a recovery.
    ///
    /// Absent is the ordinary case. Present, the instance is bootstrapped from
    /// somebody else's archive instead of starting empty -- read-only from
    /// this side, so the deployment being restored is never touched.
    #[serde(default)]
    pub restore: Option<RestorePayloadV1>,

    /// The hostname the control plane decided for this deployment --
    /// `slug(name)` under whichever domain it publishes DNS records into.
    ///
    /// Optional for the same reason `cpu_millis` is: an installation with no
    /// domain configured has none to send, and an action recorded before the
    /// control plane carried this at all looks the same. Both fall back to
    /// what genesis has always invented, in `DesiredIdentityInstance::from_payload`.
    #[serde(default)]
    pub hostname: Option<String>,
}

/// The restore half of a `deployment.restore` payload.
///
/// Both paths are the *source's*. The control plane computes them because it
/// owns the archive layout; a data plane rebuilding either would be a second
/// implementation of the prefix rule, and one that reads from the wrong prefix
/// restores somebody else's data.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RestorePayloadV1 {
    pub destination_path: String,

    /// The name barman filed the archive under, which is the source's own
    /// database cluster.
    pub server_name: String,

    #[serde(default)]
    pub backup_id: Option<String>,
}

/// The archive half of a `deployment.*` payload.
///
/// The destination is computed by the control plane, which owns the layout.
/// Everything about *reaching* the store -- endpoint, credentials -- is the
/// data plane's own configuration and deliberately does not travel: the two
/// sides can be on different networks, and a control plane dictating an
/// endpoint it cannot itself verify is a URL nobody notices is wrong until an
/// archive is due.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ArchivePayloadV1 {
    /// `s3://bucket/organisation/deployment`, with no trailing slash.
    pub destination_path: String,

    /// What the store is asked to do with the archive once it has it. Absent
    /// leaves it to the bucket's own policy, which is not the same as asking
    /// for nothing.
    #[serde(default)]
    pub encryption: Option<String>,

    pub schedule: ArchiveSchedulePayloadV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ArchiveSchedulePayloadV1 {
    /// Six fields with seconds first, read in [`Self::zone`]. Never a zone
    /// prefix: CloudNativePG's webhook counts whitespace separated fields and
    /// refuses anything but five or six.
    pub cron: String,
    pub zone: String,
    pub enabled: bool,
}

/// What genesis used to hardcode for every deployment, kept only as the
/// fallback for payloads written before the control plane sent a size.
const FALLBACK_CPU_MILLIS: u32 = 500;
const FALLBACK_MEMORY_MIB: u32 = 1024;
const FALLBACK_STORAGE_GIB: u32 = 1;

impl DeploymentPayloadV1 {
    pub fn cpu_millis(&self) -> u32 {
        self.cpu_millis.unwrap_or(FALLBACK_CPU_MILLIS)
    }

    pub fn memory_mib(&self) -> u32 {
        self.memory_mib.unwrap_or(FALLBACK_MEMORY_MIB)
    }

    pub fn storage_gib(&self) -> u32 {
        self.storage_gib.unwrap_or(FALLBACK_STORAGE_GIB)
    }
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

    /// The archive half, shaped the way the control plane writes it in
    /// `archive_section`. Duplicated from `aether-core` deliberately and
    /// visibly: this crate does not depend on it, and a payload that fails to
    /// deserialise here costs a retry loop rather than a compile error.
    #[test]
    fn deserializes_the_archive_the_control_plane_sends() {
        let raw = json!({
            "deployment_id": "b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d",
            "dataplane_id": "1c2b3a4d-5e6f-4a7b-8c9d-0e1f2a3b4c5e",
            "organisation_id": "9f8e7d6c-5b4a-3c2d-1e0f-a1b2c3d4e5f6",
            "name": "acme-prod",
            "kind": "keycloak",
            "version": "25.0.0",
            "namespace": "aether-acme-prod",
            "created_by": "11111111-2222-3333-4444-555555555555",
            "archive": {
                "destination_path": "s3://aether-backups/9f8e7d6c-5b4a-3c2d-1e0f-a1b2c3d4e5f6/b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d",
                "encryption": "AES256",
                "schedule": { "cron": "0 30 2 * * *", "zone": "UTC", "enabled": true }
            }
        });

        let archive = DeploymentPayloadV1::from_value(&raw)
            .expect("valid payload")
            .archive
            .expect("the deployment archives");

        assert!(archive.destination_path.starts_with("s3://aether-backups/"));
        assert!(!archive.destination_path.ends_with('/'));
        assert_eq!(archive.encryption.as_deref(), Some("AES256"));
        assert_eq!(archive.schedule.cron, "0 30 2 * * *");
        assert!(archive.schedule.enabled);
    }

    /// An installation that archives nowhere, and an action recorded before
    /// the control plane carried this at all, look the same here on purpose:
    /// the queue can hold events older than the field.
    #[test]
    fn a_payload_without_an_archive_still_deserializes() {
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

        assert!(
            DeploymentPayloadV1::from_value(&raw)
                .expect("valid payload")
                .archive
                .is_none()
        );
    }

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
