use crate::domain::entities::dataplane_upgrade_payload::{
    DataplaneUpgradePayloadV1, UpgradeComponent, UpgradeStrategy,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesiredDataplaneUpgrade {
    pub name: String,
    pub namespace: String,
    pub dataplane_id: String,
    pub target_version: String,
    pub components: Vec<UpgradeComponent>,
    pub strategy: UpgradeStrategy,
    pub max_unavailable: u32,
}

impl DesiredDataplaneUpgrade {
    pub fn from_payload(payload: &DataplaneUpgradePayloadV1, namespace: &str) -> Self {
        Self {
            name: payload.dataplane_id.to_string(),
            namespace: namespace.to_string(),
            dataplane_id: payload.dataplane_id.to_string(),
            target_version: payload.target_version.clone(),
            components: payload.components.clone(),
            strategy: payload.strategy,
            max_unavailable: payload.max_unavailable,
        }
    }

    pub fn reference(&self) -> DataplaneUpgradeRef {
        DataplaneUpgradeRef {
            name: self.name.clone(),
            namespace: self.namespace.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataplaneUpgradeRef {
    pub name: String,
    pub namespace: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingDataplaneUpgrade {
    pub target_version: String,
    pub finished: bool,
    pub succeeded: bool,
}
