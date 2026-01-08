use std::sync::Arc;

use aether_auth::Identity;

use crate::{
    domain::{
        CoreError,
        organisation::{
            Organisation, OrganisationId,
            commands::{
                CreateOrganisationCommand, CreateOrganisationData, UpdateOrganisationCommand,
            },
            ports::{OrganisationRepository, OrganisationService},
        },
    },
    organisation::value_objects::OrganisationStatus,
    user::UserId,
};

/// Maximum number of organisations a user can own
const MAX_ORGANISATIONS_PER_USER: usize = 10;

#[derive(Clone, Debug)]
pub struct OrganisationServiceImpl<O>
where
    O: OrganisationRepository,
{
    organisation_repository: Arc<O>,
}

impl<O> OrganisationServiceImpl<O>
where
    O: OrganisationRepository,
{
    pub fn new(organisation_repository: Arc<O>) -> Self {
        Self {
            organisation_repository,
        }
    }
}

impl<O> OrganisationService for OrganisationServiceImpl<O>
where
    O: OrganisationRepository,
{
    async fn create_organisation(
        &self,
        command: CreateOrganisationCommand,
    ) -> Result<Organisation, CoreError> {
        // 1. Check user organisation limit (max 10 organisations per user)
        let user_organisations = self
            .organisation_repository
            .find_by_owner(&command.owner_id)
            .await?;

        let active_orgs_count = user_organisations
            .iter()
            .filter(|org| org.is_active())
            .count();

        if active_orgs_count >= MAX_ORGANISATIONS_PER_USER {
            return Err(CoreError::UserOrganisationLimitReached {
                max: MAX_ORGANISATIONS_PER_USER,
                current: active_orgs_count,
            });
        }

        // 2. Generate slug if not provided
        let slug = command.get_or_generate_slug()?;

        // 3. Check if slug already exists (business rule: slugs must be unique)
        if self.organisation_repository.slug_exists(&slug).await? {
            return Err(CoreError::OrganisationSlugAlreadyExists {
                slug: slug.to_string(),
            });
        }

        let user_id = command.owner_id;
        // 4. Convert command to data
        let data = CreateOrganisationData::from_command(command)?;

        // 5. Create organisation via repository
        let organisation = self.organisation_repository.create(data).await?;

        self.organisation_repository
            .insert_member(&organisation.id, &user_id)
            .await?;

        Ok(organisation)
    }

    async fn update_organisation(
        &self,
        id: OrganisationId,
        command: UpdateOrganisationCommand,
    ) -> Result<Organisation, CoreError> {
        // 1. Validate command is not empty
        if command.is_empty() {
            return Err(CoreError::InternalError(
                "Update command cannot be empty".to_string(),
            ));
        }

        // 2. Fetch existing organisation
        let mut organisation = self
            .organisation_repository
            .find_by_id(&id)
            .await?
            .ok_or(CoreError::OrganisationNotFound { id: *id.as_uuid() })?;

        // 3. Check if organisation is active (business rule: only active orgs can be updated)
        if !organisation.is_active() {
            return Err(CoreError::OrganisationSuspended {
                reason: "Cannot update a non-active organisation".to_string(),
            });
        }

        // 4. Handle slug change if provided
        if let Some(new_slug) = &command.slug {
            // Check if new slug is different and not already taken
            if &organisation.slug != new_slug
                && self.organisation_repository.slug_exists(new_slug).await?
            {
                return Err(CoreError::OrganisationSlugAlreadyExists {
                    slug: new_slug.to_string(),
                });
            }
        }

        // 5. Apply updates to the organisation
        match (command.name, command.slug) {
            (Some(name), Some(slug)) => {
                organisation.update_name(name, slug);
            }
            (Some(name), None) => {
                // Keep existing slug
                let slug = organisation.slug.clone();
                organisation.update_name(name, slug);
            }
            (None, Some(slug)) => {
                // Keep existing name
                let name = organisation.name.clone();
                organisation.update_name(name, slug);
            }
            (None, None) => {
                // This shouldn't happen due to is_empty() check, but handle it anyway
                return Err(CoreError::InternalError("No fields to update".to_string()));
            }
        }

        let updated = self.organisation_repository.update(organisation).await?;

        Ok(updated)
    }

    async fn delete_organisation(&self, id: OrganisationId) -> Result<(), CoreError> {
        // 1. Fetch existing organisation
        let mut organisation = self
            .organisation_repository
            .find_by_id(&id)
            .await?
            .ok_or(CoreError::OrganisationNotFound { id: *id.as_uuid() })?;

        // 2. Business rule: Check if organisation is already deleted
        if organisation.is_deleted() {
            return Err(CoreError::InternalError(
                "Organisation is already deleted".to_string(),
            ));
        }

        // 3. Mark organisation as deleted (soft delete)
        organisation.delete()?;

        // 4. Persist deletion to repository
        self.organisation_repository.delete(&id).await?;

        Ok(())
    }

    async fn get_organisations(
        &self,
        status: Option<OrganisationStatus>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Organisation>, CoreError> {
        self.organisation_repository
            .list(status, limit, offset)
            .await
    }

    async fn get_organisations_by_member(
        &self,
        identity: Identity,
    ) -> Result<Vec<Organisation>, CoreError> {
        let user_id: UserId = identity
            .id()
            .parse::<UserId>()
            .map_err(|_| CoreError::InvalidIdentity)?;

        self.organisation_repository.find_by_member(&user_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        organisation::{
            ports::MockOrganisationRepository,
            value_objects::{OrganisationName, OrganisationSlug, OrganisationStatus, Plan},
        },
        user::UserId,
    };
    use chrono::Utc;
    use uuid::Uuid;

    fn create_test_organisation(
        name: &str,
        slug: &str,
        owner_id: UserId,
        plan: Plan,
    ) -> Organisation {
        Organisation {
            id: OrganisationId::new(),
            name: OrganisationName::new(name).unwrap(),
            slug: OrganisationSlug::new(slug).unwrap(),
            owner_id,
            status: OrganisationStatus::Active,
            plan,
            limits: crate::domain::organisation::value_objects::OrganisationLimits::from_plan(
                &plan,
            ),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deleted_at: None,
        }
    }

    #[tokio::test]
    async fn test_create_organisation_success() {
        let mut mock_repo = MockOrganisationRepository::new();
        let owner_id = UserId(Uuid::new_v4());
        let name = OrganisationName::new("Test Org").unwrap();

        mock_repo
            .expect_find_by_owner()
            .times(1)
            .returning(|_| Box::pin(async move { Ok(vec![]) }));

        mock_repo
            .expect_slug_exists()
            .times(1)
            .returning(|_| Box::pin(async move { Ok(false) }));

        let expected_org = create_test_organisation("Test Org", "test-org", owner_id, Plan::Free);
        mock_repo.expect_create().times(1).returning(move |_| {
            let org = expected_org.clone();
            Box::pin(async move { Ok(org) })
        });

        let service = OrganisationServiceImpl::new(Arc::new(mock_repo));
        let command = CreateOrganisationCommand::new(name, owner_id, Plan::Free);

        let result = service.create_organisation(command).await;
        assert!(result.is_ok());
        let org = result.unwrap();
        assert_eq!(org.name.as_str(), "Test Org");
        assert_eq!(org.slug.as_str(), "test-org");
    }

    #[tokio::test]
    async fn test_create_organisation_user_limit_reached() {
        let mut mock_repo = MockOrganisationRepository::new();
        let owner_id = UserId(Uuid::new_v4());
        let name = OrganisationName::new("Test Org").unwrap();

        let existing_orgs: Vec<Organisation> = (0..10)
            .map(|i| {
                create_test_organisation(
                    &format!("Org {}", i),
                    &format!("org-{}", i),
                    owner_id,
                    Plan::Free,
                )
            })
            .collect();

        mock_repo
            .expect_find_by_owner()
            .times(1)
            .returning(move |_| {
                let orgs = existing_orgs.clone();
                Box::pin(async move { Ok(orgs) })
            });

        let service = OrganisationServiceImpl::new(Arc::new(mock_repo));
        let command = CreateOrganisationCommand::new(name, owner_id, Plan::Free);

        let result = service.create_organisation(command).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            CoreError::UserOrganisationLimitReached { max, current } => {
                assert_eq!(max, 10);
                assert_eq!(current, 10);
            }
            _ => panic!("Expected UserOrganisationLimitReached error"),
        }
    }

    #[tokio::test]
    async fn test_create_organisation_slug_exists() {
        let mut mock_repo = MockOrganisationRepository::new();
        let owner_id = UserId(Uuid::new_v4());
        let name = OrganisationName::new("Test Org").unwrap();

        mock_repo
            .expect_find_by_owner()
            .times(1)
            .returning(|_| Box::pin(async move { Ok(vec![]) }));

        mock_repo
            .expect_slug_exists()
            .times(1)
            .returning(|_| Box::pin(async move { Ok(true) }));

        let service = OrganisationServiceImpl::new(Arc::new(mock_repo));
        let command = CreateOrganisationCommand::new(name, owner_id, Plan::Free);

        let result = service.create_organisation(command).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            CoreError::OrganisationSlugAlreadyExists { slug } => {
                assert_eq!(slug, "test-org");
            }
            _ => panic!("Expected OrganisationSlugAlreadyExists error"),
        }
    }

    #[tokio::test]
    async fn test_create_organisation_with_custom_slug() {
        let mut mock_repo = MockOrganisationRepository::new();
        let owner_id = UserId(Uuid::new_v4());
        let name = OrganisationName::new("Test Org").unwrap();
        let custom_slug = OrganisationSlug::new("custom-slug").unwrap();

        mock_repo
            .expect_find_by_owner()
            .times(1)
            .returning(|_| Box::pin(async move { Ok(vec![]) }));

        mock_repo
            .expect_slug_exists()
            .times(1)
            .returning(|_| Box::pin(async move { Ok(false) }));

        let expected_org =
            create_test_organisation("Test Org", "custom-slug", owner_id, Plan::Free);
        mock_repo.expect_create().times(1).returning(move |_| {
            let org = expected_org.clone();
            Box::pin(async move { Ok(org) })
        });

        let service = OrganisationServiceImpl::new(Arc::new(mock_repo));
        let command =
            CreateOrganisationCommand::new(name, owner_id, Plan::Free).with_slug(custom_slug);

        let result = service.create_organisation(command).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().slug.as_str(), "custom-slug");
    }

    #[tokio::test]
    async fn test_update_organisation_success() {
        let mut mock_repo = MockOrganisationRepository::new();
        let org_id = OrganisationId::new();
        let owner_id = UserId(Uuid::new_v4());

        let existing_org = create_test_organisation("Old Name", "old-slug", owner_id, Plan::Free);
        let updated_org = create_test_organisation("New Name", "new-slug", owner_id, Plan::Free);

        mock_repo.expect_find_by_id().times(1).returning(move |_| {
            let org = existing_org.clone();
            Box::pin(async move { Ok(Some(org)) })
        });

        mock_repo
            .expect_slug_exists()
            .times(1)
            .returning(|_| Box::pin(async move { Ok(false) }));

        mock_repo.expect_update().times(1).returning(move |_| {
            let org = updated_org.clone();
            Box::pin(async move { Ok(org) })
        });

        let service = OrganisationServiceImpl::new(Arc::new(mock_repo));
        let command = UpdateOrganisationCommand::new()
            .with_name(OrganisationName::new("New Name").unwrap())
            .with_slug(OrganisationSlug::new("new-slug").unwrap());

        let result = service.update_organisation(org_id, command).await;
        assert!(result.is_ok());
        let org = result.unwrap();
        assert_eq!(org.name.as_str(), "New Name");
        assert_eq!(org.slug.as_str(), "new-slug");
    }

    #[tokio::test]
    async fn test_update_organisation_not_found() {
        let mut mock_repo = MockOrganisationRepository::new();
        let org_id = OrganisationId::new();

        mock_repo
            .expect_find_by_id()
            .times(1)
            .returning(|_| Box::pin(async move { Ok(None) }));

        let service = OrganisationServiceImpl::new(Arc::new(mock_repo));
        let command =
            UpdateOrganisationCommand::new().with_name(OrganisationName::new("New Name").unwrap());

        let result = service.update_organisation(org_id, command).await;
        assert!(result.is_err());
        matches!(result.unwrap_err(), CoreError::OrganisationNotFound { .. });
    }

    #[tokio::test]
    async fn test_update_organisation_suspended() {
        let mut mock_repo = MockOrganisationRepository::new();
        let org_id = OrganisationId::new();
        let owner_id = UserId(Uuid::new_v4());

        let mut suspended_org = create_test_organisation("Test", "test", owner_id, Plan::Free);
        suspended_org.status = OrganisationStatus::Suspended;

        mock_repo.expect_find_by_id().times(1).returning(move |_| {
            let org = suspended_org.clone();
            Box::pin(async move { Ok(Some(org)) })
        });

        let service = OrganisationServiceImpl::new(Arc::new(mock_repo));
        let command =
            UpdateOrganisationCommand::new().with_name(OrganisationName::new("New Name").unwrap());

        let result = service.update_organisation(org_id, command).await;
        assert!(result.is_err());
        matches!(result.unwrap_err(), CoreError::OrganisationSuspended { .. });
    }

    #[tokio::test]
    async fn test_delete_organisation_success() {
        let mut mock_repo = MockOrganisationRepository::new();
        let org_id = OrganisationId::new();
        let owner_id = UserId(Uuid::new_v4());

        let existing_org = create_test_organisation("Test", "test", owner_id, Plan::Free);

        mock_repo.expect_find_by_id().times(1).returning(move |_| {
            let org = existing_org.clone();
            Box::pin(async move { Ok(Some(org)) })
        });

        mock_repo
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async move { Ok(()) }));

        let service = OrganisationServiceImpl::new(Arc::new(mock_repo));
        let result = service.delete_organisation(org_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delete_organisation_not_found() {
        let mut mock_repo = MockOrganisationRepository::new();
        let org_id = OrganisationId::new();

        mock_repo
            .expect_find_by_id()
            .times(1)
            .returning(|_| Box::pin(async move { Ok(None) }));

        let service = OrganisationServiceImpl::new(Arc::new(mock_repo));
        let result = service.delete_organisation(org_id).await;
        assert!(result.is_err());
        matches!(result.unwrap_err(), CoreError::OrganisationNotFound { .. });
    }

    #[tokio::test]
    async fn test_delete_organisation_already_deleted() {
        let mut mock_repo = MockOrganisationRepository::new();
        let org_id = OrganisationId::new();
        let owner_id = UserId(Uuid::new_v4());

        let mut deleted_org = create_test_organisation("Test", "test", owner_id, Plan::Free);
        deleted_org.status = OrganisationStatus::Deleted;
        deleted_org.deleted_at = Some(Utc::now());

        mock_repo.expect_find_by_id().times(1).returning(move |_| {
            let org = deleted_org.clone();
            Box::pin(async move { Ok(Some(org)) })
        });

        let service = OrganisationServiceImpl::new(Arc::new(mock_repo));
        let result = service.delete_organisation(org_id).await;
        assert!(result.is_err());
        matches!(result.unwrap_err(), CoreError::InternalError { .. });
    }

    #[test]
    fn test_create_command_without_slug() {
        let name = OrganisationName::new("Test Org").unwrap();
        let owner_id = UserId(Uuid::new_v4());
        let command = CreateOrganisationCommand::new(name, owner_id, Plan::Free);

        assert!(command.slug.is_none());
        assert!(command.get_or_generate_slug().is_ok());
        assert_eq!(command.get_or_generate_slug().unwrap().as_str(), "test-org");
    }

    #[test]
    fn test_create_command_with_slug() {
        let name = OrganisationName::new("Test Org").unwrap();
        let slug = OrganisationSlug::new("custom-slug").unwrap();
        let owner_id = UserId(Uuid::new_v4());
        let command =
            CreateOrganisationCommand::new(name, owner_id, Plan::Free).with_slug(slug.clone());

        assert!(command.slug.is_some());
        assert_eq!(command.get_or_generate_slug().unwrap(), slug);
    }

    #[test]
    fn test_update_command_builder() {
        let name = OrganisationName::new("New Name").unwrap();
        let slug = OrganisationSlug::new("new-slug").unwrap();

        let command = UpdateOrganisationCommand::new()
            .with_name(name.clone())
            .with_slug(slug.clone());

        assert_eq!(command.name.as_ref().unwrap(), &name);
        assert_eq!(command.slug.as_ref().unwrap(), &slug);
        assert!(!command.is_empty());
    }

    #[test]
    fn test_update_command_empty() {
        let command = UpdateOrganisationCommand::new();
        assert!(command.is_empty());
    }
}
