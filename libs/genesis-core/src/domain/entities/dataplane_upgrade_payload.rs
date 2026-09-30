use serde::Deserialize;
use uuid::Uuid;

use crate::domain::error::GenesisError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum UpgradeComponent {
    Herald,
    Genesis,
    Operator,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpgradeStrategy {
    Rolling,
    Canary,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DataplaneUpgradePayloadV1 {
    pub dataplane_id: Uuid,
    pub target_version: String,
    pub components: Vec<UpgradeComponent>,
    pub strategy: UpgradeStrategy,
    pub max_unavailable: u32,
}

impl DataplaneUpgradePayloadV1 {
    pub fn from_value(value: &serde_json::Value) -> Result<Self, GenesisError> {
        let payload: Self = serde_json::from_value(value.clone()).map_err(|error| {
            GenesisError::InvalidPayload {
                message: format!("not a data plane upgrade payload: {error}"),
            }
        })?;

        if payload.components.is_empty() {
            return Err(GenesisError::InvalidPayload {
                message: "a data plane upgrade names no component".to_string(),
            });
        }

        if payload.max_unavailable < 1 {
            return Err(GenesisError::InvalidPayload {
                message: "a data plane upgrade needs max_unavailable of at least 1".to_string(),
            });
        }

        Ok(payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn payload() -> serde_json::Value {
        json!({
            "dataplane_id": "00000000-0000-0000-0000-000000000002",
            "target_version": "26.1.0",
            "components": ["Herald", "Operator"],
            "strategy": "canary",
            "max_unavailable": 2,
        })
    }

    #[test]
    fn reads_what_the_control_plane_writes() {
        let parsed = DataplaneUpgradePayloadV1::from_value(&payload()).expect("a valid payload");

        assert_eq!(parsed.target_version, "26.1.0");
        assert_eq!(
            parsed.components,
            vec![UpgradeComponent::Herald, UpgradeComponent::Operator]
        );
        assert_eq!(parsed.strategy, UpgradeStrategy::Canary);
        assert_eq!(parsed.max_unavailable, 2);
    }

    #[test]
    fn refuses_empty_components() {
        let mut value = payload();
        value["components"] = json!([]);

        let error = DataplaneUpgradePayloadV1::from_value(&value).expect_err("no component");

        assert!(error.to_string().contains("no component"), "{error}");
    }

    #[test]
    fn refuses_a_max_unavailable_of_zero() {
        let mut value = payload();
        value["max_unavailable"] = json!(0);

        let error = DataplaneUpgradePayloadV1::from_value(&value).expect_err("zero");

        assert!(error.to_string().contains("max_unavailable"), "{error}");
    }

    #[test]
    fn refuses_an_unknown_component() {
        let mut value = payload();
        value["components"] = json!(["Kafka"]);

        assert!(DataplaneUpgradePayloadV1::from_value(&value).is_err());
    }
}
