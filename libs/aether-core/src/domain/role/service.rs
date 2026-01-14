use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use crate::{
    CoreError,
    organisation::OrganisationId,
    role::{
        Role, RoleId,
        commands::{CreateRoleCommand, UpdateRoleCommand},
        ports::{RoleRepository, RoleService},
    },
};

#[derive(Clone)]
pub struct RoleServiceImpl<R>
where
    R: RoleRepository,
{
    role_repository: Arc<R>,
}

impl<R> RoleServiceImpl<R>
where
    R: RoleRepository,
{
    pub fn new(role_repository: Arc<R>) -> Self {
        Self { role_repository }
    }
}

impl<R> RoleService for RoleServiceImpl<R>
where
    R: RoleRepository,
{
    async fn create_role(&self, command: CreateRoleCommand) -> Result<Role, CoreError> {
        let role = Role {
            id: RoleId(Uuid::new_v4()),
            name: command.name,
            permissions: command.permissions,
            organisation_id: command.organisation_id,
            color: command.color,
            created_at: Utc::now(),
        };

        self.role_repository.insert(role.clone()).await?;
        Ok(role)
    }

    async fn delete_role(&self, role_id: RoleId) -> Result<(), CoreError> {
        self.role_repository.delete(role_id).await
    }

    async fn get_role(&self, role_id: RoleId) -> Result<Option<Role>, CoreError> {
        self.role_repository.get_by_id(role_id).await
    }

    async fn list_roles_by_organisation(
        &self,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Role>, CoreError> {
        self.role_repository
            .list_by_organisation(organisation_id)
            .await
    }

    async fn update_role(
        &self,
        role_id: RoleId,
        command: UpdateRoleCommand,
    ) -> Result<Role, CoreError> {
        if command.is_empty() {
            return Err(CoreError::InternalError(
                "Update command cannot be empty".to_string(),
            ));
        }

        let mut role = self
            .role_repository
            .get_by_id(role_id)
            .await?
            .ok_or(CoreError::InternalError("Role not found".to_string()))?;

        if let Some(name) = command.name {
            role.name = name;
        }
        if let Some(permissions) = command.permissions {
            role.permissions = permissions;
        }
        if let Some(organisation_id) = command.organisation_id {
            role.organisation_id = Some(organisation_id);
        }
        if let Some(color) = command.color {
            role.color = Some(color);
        }

        self.role_repository.update(role.clone()).await?;
        Ok(role)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::role::ports::MockRoleRepository;

    fn sample_role(role_id: RoleId, organisation_id: Option<OrganisationId>) -> Role {
        Role {
            id: role_id,
            name: "admin".to_string(),
            permissions: 7,
            organisation_id,
            color: Some("#ff0000".to_string()),
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn create_role_persists_role() {
        let mut mock_repo = MockRoleRepository::new();
        mock_repo
            .expect_insert()
            .times(1)
            .withf(|role| role.name == "admin" && role.permissions == 7)
            .returning(|_| Box::pin(async { Ok(()) }));

        let service = RoleServiceImpl::new(Arc::new(mock_repo));
        let command = CreateRoleCommand::new("admin".to_string(), 7);

        let result = service.create_role(command).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "admin");
    }

    #[tokio::test]
    async fn update_role_rejects_empty_command() {
        let service = RoleServiceImpl::new(Arc::new(MockRoleRepository::new()));
        let result = service
            .update_role(RoleId(Uuid::new_v4()), UpdateRoleCommand::new())
            .await;

        assert!(matches!(result, Err(CoreError::InternalError(_))));
    }

    #[tokio::test]
    async fn update_role_applies_changes() {
        let mut mock_repo = MockRoleRepository::new();
        let role_id = RoleId(Uuid::new_v4());
        let organisation_id = OrganisationId(Uuid::new_v4());
        let existing_role = sample_role(role_id, None);

        mock_repo.expect_get_by_id().times(1).returning(move |_| {
            let role = existing_role.clone();
            Box::pin(async move { Ok(Some(role)) })
        });

        mock_repo
            .expect_update()
            .times(1)
            .withf(move |role| {
                role.name == "viewer"
                    && role.permissions == 1
                    && role.organisation_id == Some(organisation_id)
                    && role.color.as_deref() == Some("#00ff00")
            })
            .returning(|_| Box::pin(async { Ok(()) }));

        let service = RoleServiceImpl::new(Arc::new(mock_repo));
        let command = UpdateRoleCommand::new()
            .with_name("viewer".to_string())
            .with_permissions(1)
            .with_organisation_id(organisation_id)
            .with_color("#00ff00".to_string());

        let result = service.update_role(role_id, command).await;
        assert!(result.is_ok());
        let role = result.unwrap();
        assert_eq!(role.name, "viewer");
        assert_eq!(role.permissions, 1);
        assert_eq!(role.organisation_id, Some(organisation_id));
    }

    #[tokio::test]
    async fn update_role_returns_error_when_missing() {
        let mut mock_repo = MockRoleRepository::new();
        mock_repo
            .expect_get_by_id()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));

        let service = RoleServiceImpl::new(Arc::new(mock_repo));
        let command = UpdateRoleCommand::new().with_name("viewer".to_string());

        let result = service.update_role(RoleId(Uuid::new_v4()), command).await;
        assert!(matches!(result, Err(CoreError::InternalError(_))));
    }

    #[tokio::test]
    async fn list_roles_by_organisation_delegates_to_repository() {
        let mut mock_repo = MockRoleRepository::new();
        let organisation_id = OrganisationId(Uuid::new_v4());
        let roles = vec![sample_role(RoleId(Uuid::new_v4()), Some(organisation_id))];

        mock_repo
            .expect_list_by_organisation()
            .times(1)
            .returning(move |_| {
                let roles = roles.clone();
                Box::pin(async move { Ok(roles) })
            });

        let service = RoleServiceImpl::new(Arc::new(mock_repo));
        let result = service.list_roles_by_organisation(organisation_id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }
}
