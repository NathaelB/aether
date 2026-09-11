use aether_auth::Identity;
use aether_domain::{
    CoreError,
    audit::{
        AuditBatch, AuditEntry,
        commands::{ListAuditEntriesCommand, RecordAuditEntryCommand},
        ports::AuditService,
        service::AuditServiceImpl,
    },
};
use aether_macros::transactional;

use crate::{
    AetherService,
    infrastructure::role::{PostgresRoleRepository, RolePermissionProvider},
};

impl AuditService for AetherService {
    #[transactional(audit)]
    async fn record(&self, command: RecordAuditEntryCommand) -> Result<AuditEntry, CoreError> {
        AuditServiceImpl::new(
            audit_repository,
            RolePermissionProvider::new(PostgresRoleRepository::new(&tx)),
        )
        .record(command)
        .await
    }

    #[transactional(audit)]
    async fn list_entries(
        &self,
        identity: Identity,
        command: ListAuditEntriesCommand,
    ) -> Result<AuditBatch, CoreError> {
        AuditServiceImpl::new(
            audit_repository,
            RolePermissionProvider::new(PostgresRoleRepository::new(&tx)),
        )
        .list_entries(identity, command)
        .await
    }
}
