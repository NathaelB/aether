use std::fmt;

use aether_domain::action::ActionPayload;
use aether_domain::dataplane::value_objects::DataPlaneId;
use serde::{Deserialize, Serialize};

pub const PAYLOAD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpgradeComponent {
    Herald,
    Genesis,
    Operator,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpgradeStrategy {
    #[default]
    Rolling,
    Canary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidDataplaneUpgrade {
    NoComponents,
    NothingMayBeUnavailable,
}

impl fmt::Display for InvalidDataplaneUpgrade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoComponents => f.write_str("a data plane upgrade names no component"),
            Self::NothingMayBeUnavailable => {
                f.write_str("a data plane upgrade needs max_unavailable of at least 1")
            }
        }
    }
}

impl std::error::Error for InvalidDataplaneUpgrade {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataplaneUpgradePayload {
    dataplane_id: DataPlaneId,
    target_version: String,
    components: Vec<UpgradeComponent>,
    strategy: UpgradeStrategy,
    max_unavailable: u32,
}

impl DataplaneUpgradePayload {
    pub fn new(
        dataplane_id: DataPlaneId,
        target_version: String,
        components: Vec<UpgradeComponent>,
        strategy: UpgradeStrategy,
        max_unavailable: u32,
    ) -> Result<Self, InvalidDataplaneUpgrade> {
        if components.is_empty() {
            return Err(InvalidDataplaneUpgrade::NoComponents);
        }
        if max_unavailable < 1 {
            return Err(InvalidDataplaneUpgrade::NothingMayBeUnavailable);
        }

        Ok(Self {
            dataplane_id,
            target_version,
            components,
            strategy,
            max_unavailable,
        })
    }

    pub fn into_action_payload(self) -> ActionPayload {
        ActionPayload {
            data: serde_json::to_value(self).expect("plain fields always serialise"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use uuid::Uuid;

    fn payload() -> DataplaneUpgradePayload {
        DataplaneUpgradePayload::new(
            DataPlaneId(Uuid::nil()),
            "26.1.0".to_string(),
            vec![UpgradeComponent::Herald, UpgradeComponent::Genesis],
            UpgradeStrategy::Canary,
            2,
        )
        .expect("a valid payload")
    }

    #[test]
    fn the_payload_is_snake_case_with_the_crd_spelling_of_its_values() {
        let value = payload().into_action_payload().data;

        assert_eq!(
            value,
            json!({
                "dataplane_id": Uuid::nil(),
                "target_version": "26.1.0",
                "components": ["Herald", "Genesis"],
                "strategy": "canary",
                "max_unavailable": 2,
            })
        );
    }

    #[test]
    fn the_payload_round_trips_through_json() {
        let original = payload();

        let value = original.clone().into_action_payload().data;
        let back: DataplaneUpgradePayload = serde_json::from_value(value).unwrap();

        assert_eq!(back, original);
    }

    #[test]
    fn empty_components_are_rejected() {
        let result = DataplaneUpgradePayload::new(
            DataPlaneId(Uuid::nil()),
            "26.1.0".to_string(),
            vec![],
            UpgradeStrategy::Rolling,
            1,
        );

        assert_eq!(result, Err(InvalidDataplaneUpgrade::NoComponents));
    }

    #[test]
    fn a_max_unavailable_of_zero_is_rejected() {
        let result = DataplaneUpgradePayload::new(
            DataPlaneId(Uuid::nil()),
            "26.1.0".to_string(),
            vec![UpgradeComponent::All],
            UpgradeStrategy::Rolling,
            0,
        );

        assert_eq!(
            result,
            Err(InvalidDataplaneUpgrade::NothingMayBeUnavailable)
        );
    }
}
