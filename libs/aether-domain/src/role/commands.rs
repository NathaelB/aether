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

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn create_role_command_defaults_optional_fields() {
        let command = CreateRoleCommand::new("admin".to_string(), 42);

        assert_eq!(command.name, "admin");
        assert_eq!(command.permissions, 42);
        assert!(command.organisation_id.is_none());
        assert!(command.color.is_none());
    }

    #[test]
    fn create_role_command_builder_sets_optional_fields() {
        let organisation_id = OrganisationId(Uuid::new_v4());
        let command = CreateRoleCommand::new("editor".to_string(), 7)
            .with_organisation_id(organisation_id)
            .with_color("#ffcc00".to_string());

        assert_eq!(command.organisation_id, Some(organisation_id));
        assert_eq!(command.color.as_deref(), Some("#ffcc00"));
    }

    #[test]
    fn update_role_command_is_empty_when_no_fields_set() {
        let command = UpdateRoleCommand::new();

        assert!(command.is_empty());
    }

    #[test]
    fn update_role_command_builder_sets_fields() {
        let organisation_id = OrganisationId(Uuid::new_v4());
        let command = UpdateRoleCommand::new()
            .with_name("viewer".to_string())
            .with_permissions(1)
            .with_organisation_id(organisation_id)
            .with_color("#00aaff".to_string());

        assert_eq!(command.name.as_deref(), Some("viewer"));
        assert_eq!(command.permissions, Some(1));
        assert_eq!(command.organisation_id, Some(organisation_id));
        assert_eq!(command.color.as_deref(), Some("#00aaff"));
        assert!(!command.is_empty());
    }
}
