use crate::organisation::OrganisationId;

/// Command to create a new role
#[derive(Debug, Clone)]
pub struct CreateRoleCommand {
    pub name: String,
    pub permissions: u64,
    pub organisation_id: Option<OrganisationId>,
    pub color: Option<String>,
}

impl CreateRoleCommand {
    pub fn new(name: String, permissions: u64) -> Self {
        Self {
            name,
            permissions,
            organisation_id: None,
            color: None,
        }
    }

    pub fn with_organisation_id(mut self, organisation_id: OrganisationId) -> Self {
        self.organisation_id = Some(organisation_id);
        self
    }

    pub fn with_color(mut self, color: String) -> Self {
        self.color = Some(color);
        self
    }
}

/// Command to update an existing role
#[derive(Debug, Clone, Default)]
pub struct UpdateRoleCommand {
    pub name: Option<String>,
    pub permissions: Option<u64>,
    pub organisation_id: Option<OrganisationId>,
    pub color: Option<String>,
}

impl UpdateRoleCommand {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn with_permissions(mut self, permissions: u64) -> Self {
        self.permissions = Some(permissions);
        self
    }

    pub fn with_organisation_id(mut self, organisation_id: OrganisationId) -> Self {
        self.organisation_id = Some(organisation_id);
        self
    }

    pub fn with_color(mut self, color: String) -> Self {
        self.color = Some(color);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.permissions.is_none()
            && self.organisation_id.is_none()
            && self.color.is_none()
    }
}
