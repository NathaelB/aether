use uuid::Uuid;

use crate::domain::entities::deployment_payload::DeploymentPayloadV1;
use crate::domain::error::GenesisError;

/// Identifies the `IdentityInstance` custom resource that corresponds to a deployment.
///
/// The name is derived deterministically from `deployment_id` so that redelivering the
/// same event (create, update, or a retry after a lost ack) always targets the same
/// resource instead of minting a new one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdentityInstanceRef {
    pub name: String,
    pub namespace: String,
}

impl IdentityInstanceRef {
    pub fn for_deployment(deployment_id: Uuid, namespace: impl Into<String>) -> Self {
        Self {
            name: format!("deployment-{deployment_id}"),
            namespace: namespace.into(),
        }
    }
}

/// Identity provider to deploy, decoded from the payload's `kind` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityInstanceProvider {
    Keycloak,
    Ferriskey,
}

impl IdentityInstanceProvider {
    pub fn parse(kind: &str) -> Result<Self, GenesisError> {
        match kind.to_ascii_lowercase().as_str() {
            "keycloak" => Ok(Self::Keycloak),
            "ferriskey" => Ok(Self::Ferriskey),
            other => Err(GenesisError::InvalidPayload {
                message: format!("unknown deployment kind `{other}`"),
            }),
        }
    }
}

/// Database sizing for the `IdentityInstance`. The `deployment.*` payload carries no
/// sizing information today, so these are fixed, conservative defaults applied to every
/// instance genesis creates. See the workstream report's "Open questions" for the
/// follow-up needed to make this configurable per deployment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesiredDatabase {
    pub instances: i32,
    pub storage_size: String,
    pub cpu_request: String,
    pub memory_request: String,
    pub cpu_limit: String,
    pub memory_limit: String,
}

impl Default for DesiredDatabase {
    fn default() -> Self {
        Self {
            instances: 1,
            storage_size: "1Gi".to_string(),
            cpu_request: "250m".to_string(),
            memory_request: "512Mi".to_string(),
            cpu_limit: "1".to_string(),
            memory_limit: "1Gi".to_string(),
        }
    }
}

/// The desired state of an `IdentityInstance`, in domain terms. The adapter maps this to
/// the actual `aether_crds::v1alpha::identity_instance::IdentityInstance` custom resource.
#[derive(Debug, Clone, PartialEq)]
pub struct DesiredIdentityInstance {
    pub reference: IdentityInstanceRef,
    pub organisation_id: String,
    pub provider: IdentityInstanceProvider,
    pub version: String,
    pub hostname: String,
    pub database: DesiredDatabase,
}

impl DesiredIdentityInstance {
    pub fn from_payload(payload: &DeploymentPayloadV1) -> Result<Self, GenesisError> {
        let provider = IdentityInstanceProvider::parse(&payload.kind)?;
        let reference =
            IdentityInstanceRef::for_deployment(payload.deployment_id, payload.namespace.clone());
        // Best-effort hostname derived from the deployment name/namespace: the payload
        // does not carry a hostname today (see report's "Open questions").
        let hostname = format!("{}.{}.aether.local", payload.name, payload.namespace);

        Ok(Self {
            reference,
            organisation_id: payload.organisation_id.to_string(),
            provider,
            version: payload.version.clone(),
            hostname,
            database: DesiredDatabase::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload() -> DeploymentPayloadV1 {
        DeploymentPayloadV1 {
            deployment_id: Uuid::parse_str("b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d").unwrap(),
            dataplane_id: Uuid::nil(),
            organisation_id: Uuid::parse_str("9f8e7d6c-5b4a-3c2d-1e0f-a1b2c3d4e5f6").unwrap(),
            name: "acme-prod".to_string(),
            kind: "keycloak".to_string(),
            version: "25.0.0".to_string(),
            namespace: "aether-acme-prod".to_string(),
            created_by: Uuid::nil(),
        }
    }

    #[test]
    fn reference_is_deterministic_from_deployment_id() {
        let a = IdentityInstanceRef::for_deployment(
            Uuid::parse_str("b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d").unwrap(),
            "ns",
        );
        let b = IdentityInstanceRef::for_deployment(
            Uuid::parse_str("b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d").unwrap(),
            "ns",
        );

        assert_eq!(a, b);
        assert_eq!(a.name, "deployment-b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d");
    }

    #[test]
    fn provider_parses_case_insensitively() {
        assert_eq!(
            IdentityInstanceProvider::parse("Keycloak").unwrap(),
            IdentityInstanceProvider::Keycloak
        );
        assert_eq!(
            IdentityInstanceProvider::parse("ferriskey").unwrap(),
            IdentityInstanceProvider::Ferriskey
        );
        assert!(IdentityInstanceProvider::parse("bogus").is_err());
    }

    #[test]
    fn desired_state_is_built_from_payload() {
        let desired = DesiredIdentityInstance::from_payload(&payload()).unwrap();

        assert_eq!(desired.reference.namespace, "aether-acme-prod");
        assert_eq!(desired.provider, IdentityInstanceProvider::Keycloak);
        assert_eq!(desired.version, "25.0.0");
        assert_eq!(
            desired.organisation_id,
            "9f8e7d6c-5b4a-3c2d-1e0f-a1b2c3d4e5f6"
        );
        assert_eq!(desired.hostname, "acme-prod.aether-acme-prod.aether.local");
    }
}
