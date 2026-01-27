use crate::{
    CoreError,
    organisation::value_objects::{OrganisationLimits, OrganisationName, OrganisationSlug, Plan},
    user::UserId,
};

/// Command to create a new organisation
#[derive(Debug, Clone)]
pub struct CreateOrganisationCommand {
    pub name: OrganisationName,
    pub slug: Option<OrganisationSlug>,
    pub owner_sub: String,
    pub plan: Plan,
}

impl CreateOrganisationCommand {
    pub fn new(name: OrganisationName, owner_sub: String, plan: Plan) -> Self {
        Self {
            name,
            slug: None,
            owner_sub,
            plan,
        }
    }

    pub fn with_slug(mut self, slug: OrganisationSlug) -> Self {
        self.slug = Some(slug);
        self
    }

    /// Gets or generates the slug
    pub fn get_or_generate_slug(&self) -> Result<OrganisationSlug, CoreError> {
        match &self.slug {
            Some(slug) => Ok(slug.clone()),
            None => OrganisationSlug::from_name(&self.name),
        }
    }
}

/// Data structure for creating an organisation in the repository
/// This separates domain logic from persistence logic
#[derive(Debug, Clone)]
pub struct CreateOrganisationData {
    pub name: OrganisationName,
    pub slug: OrganisationSlug,
    pub owner_id: UserId,
    pub plan: Plan,
    pub limits: OrganisationLimits,
}

impl CreateOrganisationData {
    pub fn new(
        name: OrganisationName,
        slug: OrganisationSlug,
        owner_id: UserId,
        plan: Plan,
        limits: OrganisationLimits,
    ) -> Self {
        Self {
            name,
            slug,
            owner_id,
            plan,
            limits,
        }
    }

    pub fn from_command(
        command: CreateOrganisationCommand,
        owner_id: UserId,
    ) -> Result<Self, CoreError> {
        let slug = command.get_or_generate_slug()?;
        let limits = OrganisationLimits::from_plan(&command.plan);

        Ok(Self {
            name: command.name,
            slug,
            owner_id,
            plan: command.plan,
            limits,
        })
    }
}

/// Command to update an organisation
#[derive(Debug, Clone)]
pub struct UpdateOrganisationCommand {
    pub name: Option<OrganisationName>,
    pub slug: Option<OrganisationSlug>,
}

impl UpdateOrganisationCommand {
    pub fn new() -> Self {
        Self {
            name: None,
            slug: None,
        }
    }

    pub fn with_name(mut self, name: OrganisationName) -> Self {
        self.name = Some(name);
        self
    }

    pub fn with_slug(mut self, slug: OrganisationSlug) -> Self {
        self.slug = Some(slug);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.slug.is_none()
    }
}

impl Default for UpdateOrganisationCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organisation::value_objects::{
        OrganisationLimits, OrganisationName, OrganisationSlug, Plan,
    };
    use uuid::Uuid;

    fn user_sub() -> String {
        "user-sub-test".to_string()
    }

    #[test]
    fn get_or_generate_slug_prefers_provided_slug() {
        let name = OrganisationName::new("Acme Corp").unwrap();
        let slug = OrganisationSlug::new("custom-slug").unwrap();
        let command =
            CreateOrganisationCommand::new(name, user_sub(), Plan::Free).with_slug(slug.clone());

        let result = command.get_or_generate_slug().unwrap();
        assert_eq!(result, slug);
    }

    #[test]
    fn get_or_generate_slug_generates_from_name() {
        let name = OrganisationName::new("Acme Corp!").unwrap();
        let command = CreateOrganisationCommand::new(name, user_sub(), Plan::Starter);

        let result = command.get_or_generate_slug().unwrap();
        assert_eq!(result.as_str(), "acme-corp");
    }

    #[test]
    fn create_organisation_data_from_command_sets_limits() {
        let name = OrganisationName::new("Acme Corp").unwrap();
        let command = CreateOrganisationCommand::new(name, user_sub(), Plan::Business)
            .with_slug(OrganisationSlug::new("acme").unwrap());

        let owner_id = UserId(Uuid::new_v4());
        let data = CreateOrganisationData::from_command(command, owner_id).unwrap();
        assert_eq!(data.limits, OrganisationLimits::from_plan(&Plan::Business));
    }

    #[test]
    fn update_command_empty_state() {
        let empty = UpdateOrganisationCommand::new();
        assert!(empty.is_empty());

        let with_name =
            UpdateOrganisationCommand::new().with_name(OrganisationName::new("Acme Corp").unwrap());
        assert!(!with_name.is_empty());
    }
}
