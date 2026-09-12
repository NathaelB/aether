use aether_auth::Identity;
use aether_domain::{
    CoreError,
    backups::{
        Backup,
        commands::{RecordArchiveCommand, RecordArchiveFailureCommand},
        ports::BackupService,
        service::BackupServiceImpl,
    },
};
use aether_macros::transactional;

use crate::AetherService;

impl BackupService for AetherService {
    /// One transaction for the lookup and the write. Two reports of the same
    /// archive arriving together would otherwise both find nothing and both
    /// insert; the unique index would catch the second, but as a database
    /// error rather than as the no-op a redelivery is.
    #[transactional(backup, deployment, audit)]
    async fn record_archive(
        &self,
        identity: Identity,
        command: RecordArchiveCommand,
    ) -> Result<Backup, CoreError> {
        BackupServiceImpl::new(backup_repository, deployment_repository, audit_repository)
            .record_archive(identity, command)
            .await
    }

    #[transactional(backup, deployment, audit)]
    async fn record_archive_failure(
        &self,
        identity: Identity,
        command: RecordArchiveFailureCommand,
    ) -> Result<(), CoreError> {
        BackupServiceImpl::new(backup_repository, deployment_repository, audit_repository)
            .record_archive_failure(identity, command)
            .await
    }
}
