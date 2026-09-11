use aether_auth::Identity;
use aether_permission::Permissions;
use chrono::Utc;
use uuid::Uuid;

use crate::{
    CoreError,
    audit::{
        AuditBatch, AuditEntry, AuditEntryId,
        commands::{ListAuditEntriesCommand, RecordAuditEntryCommand},
        ports::{AuditRepository, AuditService},
    },
    role::ports::PermissionProvider,
};

pub struct AuditServiceImpl<R, P>
where
    R: AuditRepository,
    P: PermissionProvider,
{
    audit_repository: R,
    permission_provider: P,
}

impl<R, P> AuditServiceImpl<R, P>
where
    R: AuditRepository,
    P: PermissionProvider,
{
    pub fn new(audit_repository: R, permission_provider: P) -> Self {
        Self {
            audit_repository,
            permission_provider,
        }
    }
}

impl<R, P> AuditService for AuditServiceImpl<R, P>
where
    R: AuditRepository,
    P: PermissionProvider,
{
    async fn record(&self, command: RecordAuditEntryCommand) -> Result<AuditEntry, CoreError> {
        let entry = AuditEntry::record(
            AuditEntryId(Uuid::new_v4()),
            command.organisation_id,
            command.actor,
            command.action,
            command.target,
            command.change,
            Utc::now(),
        );

        self.audit_repository.append(entry.clone()).await?;
        Ok(entry)
    }

    async fn list_entries(
        &self,
        identity: Identity,
        command: ListAuditEntriesCommand,
    ) -> Result<AuditBatch, CoreError> {
        let permissions = self
            .permission_provider
            .permissions_for_organisation(identity, command.organisation_id)
            .await?;

        if !permissions.can(Permissions::VIEW_ORGANISATION) {
            return Err(CoreError::PermissionDenied {
                reason: "viewing the audit log requires VIEW_ORGANISATION".to_string(),
            });
        }

        self.audit_repository
            .list_for_organisation(command.organisation_id, command.cursor, command.limit)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::ports::MockAuditRepository;
    use crate::audit::{AuditAction, AuditActor, AuditCursor, AuditTarget, AuditTargetKind};
    use crate::organisation::OrganisationId;
    use crate::role::ports::MockPermissionProvider;
    use aether_auth::User;

    fn identity() -> Identity {
        Identity::User(User {
            id: "user-1".to_string(),
            username: "user".to_string(),
            email: None,
            name: None,
            roles: vec![],
        })
    }

    fn allow(permissions: Permissions) -> MockPermissionProvider {
        let mut provider = MockPermissionProvider::new();
        provider
            .expect_permissions_for_organisation()
            .returning(move |_, _| {
                let permissions = permissions;
                Box::pin(async move { Ok(permissions) })
            });
        provider
    }

    fn command(organisation_id: OrganisationId) -> RecordAuditEntryCommand {
        RecordAuditEntryCommand::new(
            organisation_id,
            AuditActor::System,
            AuditAction("organisation.maintenance_window.updated".to_string()),
            AuditTarget {
                kind: AuditTargetKind::Organisation,
                id: organisation_id.0,
            },
        )
    }

    #[tokio::test]
    async fn record_appends_and_returns_the_entry() {
        let mut repository = MockAuditRepository::new();
        let organisation_id = OrganisationId(Uuid::new_v4());

        repository
            .expect_append()
            .times(1)
            .withf(move |entry| {
                entry.organisation_id == organisation_id
                    && entry.action
                        == AuditAction("organisation.maintenance_window.updated".to_string())
            })
            .returning(|_| Box::pin(async { Ok(()) }));

        let service = AuditServiceImpl::new(repository, allow(Permissions::empty()));
        let result = service.record(command(organisation_id)).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn viewing_with_the_permission_reaches_the_repository() {
        let mut repository = MockAuditRepository::new();
        let organisation_id = OrganisationId(Uuid::new_v4());
        repository
            .expect_list_for_organisation()
            .times(1)
            .returning(|_, _, _| {
                Box::pin(async {
                    Ok(AuditBatch {
                        entries: vec![],
                        next_cursor: None,
                    })
                })
            });

        let service = AuditServiceImpl::new(repository, allow(Permissions::VIEW_ORGANISATION));
        let result = service
            .list_entries(
                identity(),
                ListAuditEntriesCommand::new(organisation_id, 50),
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn viewing_without_the_permission_is_refused() {
        let repository = MockAuditRepository::new();
        let organisation_id = OrganisationId(Uuid::new_v4());

        let service = AuditServiceImpl::new(repository, allow(Permissions::empty()));
        let result = service
            .list_entries(
                identity(),
                ListAuditEntriesCommand::new(organisation_id, 50),
            )
            .await;

        assert!(matches!(result, Err(CoreError::PermissionDenied { .. })));
    }

    #[tokio::test]
    async fn an_administrator_may_view_without_the_named_permission() {
        let mut repository = MockAuditRepository::new();
        let organisation_id = OrganisationId(Uuid::new_v4());
        repository
            .expect_list_for_organisation()
            .times(1)
            .returning(|_, _, _| {
                Box::pin(async {
                    Ok(AuditBatch {
                        entries: vec![],
                        next_cursor: None,
                    })
                })
            });

        let service = AuditServiceImpl::new(repository, allow(Permissions::ADMINISTRATOR));
        let result = service
            .list_entries(
                identity(),
                ListAuditEntriesCommand::new(organisation_id, 50),
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn the_cursor_and_limit_travel_to_the_repository_unchanged() {
        let mut repository = MockAuditRepository::new();
        let organisation_id = OrganisationId(Uuid::new_v4());

        repository
            .expect_list_for_organisation()
            .times(1)
            .withf(move |id, cursor, limit| {
                *id == organisation_id
                    && *cursor == Some(AuditCursor::new("cursor-1"))
                    && *limit == 25
            })
            .returning(|_, _, _| {
                Box::pin(async {
                    Ok(AuditBatch {
                        entries: vec![],
                        next_cursor: None,
                    })
                })
            });

        let service = AuditServiceImpl::new(repository, allow(Permissions::VIEW_ORGANISATION));
        let command = ListAuditEntriesCommand::new(organisation_id, 25)
            .with_cursor(AuditCursor::new("cursor-1"));

        let result = service.list_entries(identity(), command).await;
        assert!(result.is_ok());
    }
}
