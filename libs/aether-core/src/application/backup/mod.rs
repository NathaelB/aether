use aether_auth::Identity;
use aether_domain::{
    CoreError,
    action::{
        ActionPayload, ActionSource, ActionTarget, ActionType, ActionVersion, TargetKind,
        commands::RecordActionCommand, ports::ActionService, service::ActionServiceImpl,
    },
    backups::{
        Backup, BackupSchedule,
        commands::{RecordArchiveCommand, RecordArchiveFailureCommand, SetBackupScheduleCommand},
        ports::BackupService,
        service::BackupServiceImpl,
    },
    deployments::{DeploymentId, ports::DeploymentRepository},
    organisation::OrganisationId,
};
use aether_macros::transactional;

use crate::{
    AetherService,
    application::deployment::{archive_section, deployment_payload},
    infrastructure::role::permissions_in,
    policy::AetherPolicy,
};

impl BackupService for AetherService {
    /// One transaction for the lookup and the write. Two reports of the same
    /// archive arriving together would otherwise both find nothing and both
    /// insert; the unique index would catch the second, but as a database
    /// error rather than as the no-op a redelivery is.
    #[transactional(backup, backup_schedule, deployment, audit)]
    async fn record_archive(
        &self,
        identity: Identity,
        command: RecordArchiveCommand,
    ) -> Result<Backup, CoreError> {
        BackupServiceImpl::new(
            backup_repository,
            backup_schedule_repository,
            deployment_repository,
            audit_repository,
            AetherPolicy::new(permissions_in(&tx)),
        )
        .record_archive(identity, command)
        .await
    }

    #[transactional(backup, backup_schedule, deployment, audit)]
    async fn record_archive_failure(
        &self,
        identity: Identity,
        command: RecordArchiveFailureCommand,
    ) -> Result<(), CoreError> {
        BackupServiceImpl::new(
            backup_repository,
            backup_schedule_repository,
            deployment_repository,
            audit_repository,
            AetherPolicy::new(permissions_in(&tx)),
        )
        .record_archive_failure(identity, command)
        .await
    }

    #[transactional(backup, backup_schedule, deployment, audit)]
    async fn list_backups(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<Vec<Backup>, CoreError> {
        BackupServiceImpl::new(
            backup_repository,
            backup_schedule_repository,
            deployment_repository,
            audit_repository,
            AetherPolicy::new(permissions_in(&tx)),
        )
        .list_backups(identity, organisation_id, deployment_id)
        .await
    }

    #[transactional(backup, backup_schedule, deployment, audit)]
    async fn get_backup_schedule(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<BackupSchedule, CoreError> {
        BackupServiceImpl::new(
            backup_repository,
            backup_schedule_repository,
            deployment_repository,
            audit_repository,
            AetherPolicy::new(permissions_in(&tx)),
        )
        .get_backup_schedule(identity, organisation_id, deployment_id)
        .await
    }

    #[transactional(backup, backup_schedule, deployment, audit, action)]
    async fn set_backup_schedule(
        &self,
        identity: Identity,
        command: SetBackupScheduleCommand,
    ) -> Result<BackupSchedule, CoreError> {
        let deployment_id = command.deployment_id;

        let schedule = BackupServiceImpl::new(
            backup_repository,
            backup_schedule_repository,
            aether_postgres::deployments::PostgresDeploymentRepository::new(&tx),
            audit_repository,
            AetherPolicy::new(permissions_in(&tx)),
        )
        .set_backup_schedule(identity, command)
        .await?;

        // A schedule the data plane never hears about is a row, not a
        // schedule. Recorded in the same transaction as the write it
        // describes, for the reason every other action is: one that outlives a
        // rolled-back change would have the cluster archiving on terms the
        // control plane does not believe it set.
        let deployment = deployment_repository
            .get_by_id(deployment_id)
            .await?
            .ok_or(CoreError::DeploymentNotFound {
                id: deployment_id.0,
            })?;

        let archive = self
            .archive_config()
            .destination_for(deployment.organisation_id, deployment.id)
            .map(|destination| archive_section(&destination, self.archive_encryption(), &schedule));

        // An installation that archives nowhere has nothing to tell the data
        // plane. The row stands, so turning archiving on later applies what
        // the customer already chose.
        if archive.is_some() {
            ActionServiceImpl::new(action_repository)
                .record_action(RecordActionCommand::new(
                    deployment.id,
                    deployment.dataplane_id,
                    ActionType("deployment.update".to_string()),
                    ActionTarget {
                        kind: TargetKind::Deployment,
                        id: deployment.id.0,
                    },
                    ActionPayload {
                        data: deployment_payload(&deployment, archive),
                    },
                    ActionVersion(1),
                    ActionSource::System,
                ))
                .await?;
        }

        Ok(schedule)
    }
}
