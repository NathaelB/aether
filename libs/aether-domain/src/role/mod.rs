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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn role_id_from_uuid() {
        let uuid = Uuid::new_v4();
        let role_id = RoleId::from(uuid);

        assert_eq!(role_id.0, uuid);
    }

    #[test]
    fn role_struct_holds_values() {
        let role = Role {
            id: RoleId(Uuid::new_v4()),
            name: "admin".to_string(),
            permissions: 255,
            organisation_id: None,
            color: Some("#ffffff".to_string()),
            created_at: Utc::now(),
        };

        assert_eq!(role.name, "admin");
        assert_eq!(role.permissions, 255);
        assert!(role.organisation_id.is_none());
        assert_eq!(role.color.as_deref(), Some("#ffffff"));
    }
}
