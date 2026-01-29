use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataPlaneId(pub String);

pub enum DataPlaneMode {
    Shared,
    Dedicated,
}

pub struct DataPlane {
    pub id: DataPlaneId,
    pub mode: DataPlaneMode,
}
