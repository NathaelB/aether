use crate::domain::{
    organisation::value_objects::{OrganisationLimits, OrganisationName, OrganisationSlug, Plan},
    user::UserId,
};

/// Command to create a new organisation
#[derive(Debug, Clone)]
pub struct CreateOrganisationCommand {
    pub name: OrganisationName,
    pub slug: Option<OrganisationSlug>,
    pub owner_id: UserId,
    pub plan: Plan,
}

impl CreateOrganisationCommand {
    pub fn new(name: OrganisationName, owner_id: UserId, plan: Plan) -> Self {
        Self {
            name,
            slug: None,
            owner_id,
            plan,
        }
    }

    pub fn with_slug(mut self, slug: OrganisationSlug) -> Self {
        self.slug = Some(slug);
        self
    }

    /// Gets or generates the slug
    pub fn get_or_generate_slug(&self) -> Result<OrganisationSlug, crate::domain::CoreError> {
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
    ) -> Result<Self, crate::domain::CoreError> {
        let slug = command.get_or_generate_slug()?;
        let limits = OrganisationLimits::from_plan(&command.plan);

        Ok(Self {
            name: command.name,
            slug,
            owner_id: command.owner_id,
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
