use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::organisation::OrganisationId;

pub mod commands;
pub mod ports;
pub mod service;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, ToSchema)]
pub struct RoleId(pub Uuid);

impl From<Uuid> for RoleId {
    fn from(value: Uuid) -> Self {
        RoleId(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, ToSchema, Serialize)]
pub struct Role {
    pub id: RoleId,
    pub name: String,
    pub permissions: u64,
    pub organisation_id: Option<OrganisationId>,
    pub color: Option<String>,
    pub created_at: DateTime<Utc>,
}
