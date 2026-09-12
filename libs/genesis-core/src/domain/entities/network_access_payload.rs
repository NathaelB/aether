use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::domain::error::GenesisError;

/// Decoded payload of a `deployment.network_access` action, schema version 1.
///
/// `allowed_cidrs` is null for open rather than an empty list, the same shape
/// the control plane's own type uses: there is no spelling of "reachable by
/// nobody" anywhere along this path, so no message can produce one.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NetworkAccessPayloadV1 {
    pub deployment_id: Uuid,
    pub namespace: String,
    #[serde(default)]
    pub allowed_cidrs: Option<Vec<String>>,
}

impl NetworkAccessPayloadV1 {
    pub fn from_value(payload: &Value) -> Result<Self, GenesisError> {
        serde_json::from_value(payload.clone()).map_err(|e| GenesisError::InvalidPayload {
            message: e.to_string(),
        })
    }

    pub fn ranges(&self) -> &[String] {
        self.allowed_cidrs.as_deref().unwrap_or_default()
    }
}
