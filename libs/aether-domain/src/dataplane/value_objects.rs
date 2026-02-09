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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListDataPlaneDeploymentsCommand {
    pub shard_index: usize,
    pub shard_count: usize,
    pub limit: usize,
    pub cursor: Option<String>,
}

impl ListDataPlaneDeploymentsCommand {
    pub const DEFAULT_LIMIT: usize = 10;

    pub fn new(
        shard_index: Option<usize>,
        shard_count: Option<usize>,
        limit: Option<usize>,
        cursor: Option<String>,
    ) -> Result<Self, String> {
        let shard_count = shard_count.unwrap_or(1);
        if shard_count == 0 {
            return Err("shard_count must be greater than 0".to_string());
        }

        let shard_index = shard_index.unwrap_or(0);
        if shard_index >= shard_count {
            return Err("shard_index must be lower than shard_count".to_string());
        }

        let limit = limit.unwrap_or(Self::DEFAULT_LIMIT);
        if limit == 0 {
            return Err("limit must be greater than 0".to_string());
        }

        Ok(Self {
            shard_index,
            shard_count,
            limit,
            cursor,
        })
    }
}
