use std::fmt::Display;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::CoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct DataPlaneId(pub Uuid);

impl Display for DataPlaneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct Region(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub enum DataPlaneMode {
    Shared,
    Dedicated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub enum DataPlaneStatus {
    Active,
    Draining,
    Disabled,
}

impl Region {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct Capacity {
    max: u32,
}

impl Capacity {
    pub fn new(max: u32) -> Result<Self, CoreError> {
        if max == 0 {
            return Err(CoreError::InvalidDataPlaneCapacity);
        }

        Ok(Self { max })
    }

    pub fn max(&self) -> u32 {
        self.max
    }
}

pub struct CreateDataplaneCommand {
    pub region: Region,
    pub mode: DataPlaneMode,
    pub capacity: Capacity,
}
