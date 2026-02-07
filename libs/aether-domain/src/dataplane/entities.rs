use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    dataplane::value_objects::{Capacity, DataPlaneId, DataPlaneMode, DataPlaneStatus, Region},
    generate_uuid_v7,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DataPlane {
    pub id: DataPlaneId,
    pub mode: DataPlaneMode,
    pub region: Region,
    pub status: DataPlaneStatus,
    pub capacity: Capacity,
}

impl DataPlane {
    pub fn new(mode: DataPlaneMode, region: Region, capacity: Capacity) -> Self {
        Self {
            id: DataPlaneId(generate_uuid_v7()),
            status: DataPlaneStatus::Active,
            capacity,
            mode,
            region,
        }
    }
}
