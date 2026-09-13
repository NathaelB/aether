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
        plan_restore,
        ports::{BackupScheduleRepository, BackupService},
        restore::{PlannedRestore, RestoreBackupCommand},
        service::BackupServiceImpl,
    },
    dataplane::ports::DataPlaneRepository,
    deployments::{
        Deployment, DeploymentId,
        commands::{CreateDeploymentCommand, Recovery},
        ports::{DeploymentRepository, DeploymentService},
        service::DeploymentServiceImpl,
    },
    organisation::OrganisationId,
};
use aether_macros::transactional;

use crate::{
    AetherService,
    application::deployment::{archive_section, deployment_payload},
    infrastructure::{provisioner::LocalClusterProvisioner, role::permissions_in},
    policy::AetherPolicy,
};

/// What the data plane is told to bootstrap the recovery from.
///
/// Beside the ordinary deployment payload rather than folded into it: a data
/// plane that ignores this section provisions an empty instance, which is the
/// old behaviour and not a silent half-restore of somebody's data.
fn restore_section(planned: &PlannedRestore, backup: &Backup) -> serde_json::Value {
    serde_json::json!({
        "backup_id": backup.id.0,
        "destination_path": planned.source.destination_path,
        // Barman files an archive under the cluster that wrote it. A recovery
        // pointed at the prefix but not the server finds nothing and comes up
        // empty, which looks exactly like a restore that worked.
        "server_name": planned.source.server_name,
        "postgres_major": planned.target.postgres_major.0,
    })
}

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

    /// Reading an archive, placing a second deployment, and telling the data
    /// plane where to read from -- in one transaction.
    ///
    /// One transaction because the three are one act. A recovery inserted
    /// without its action is an empty instance nobody asked for; an action
    /// recorded without its row is a data plane bootstrapping a deployment the
    /// control plane does not know about.
    #[transactional(
        backup,
        backup_schedule,
        deployment,
        audit,
        user,
        data_plane,
        action,
        organisation
    )]
    async fn restore_backup(
        &self,
        identity: Identity,
        command: RestoreBackupCommand,
    ) -> Result<Deployment, CoreError> {
        let (archive, source) = BackupServiceImpl::new(
            backup_repository,
            aether_postgres::backups::PostgresBackupScheduleRepository::new(&tx),
            aether_postgres::deployments::PostgresDeploymentRepository::new(&tx),
            audit_repository,
            AetherPolicy::new(permissions_in(&tx)),
        )
        .restorable(identity.clone(), command.organisation_id, command.backup)
        .await?;

        // Only to answer what a source that predates the catalogue is placed
        // as. The tenancy it already runs under is the one thing a recovery
        // must not change.
        let source_mode = data_plane_repository
            .find_by_id(&source.dataplane_id)
            .await?
            .ok_or(CoreError::DataPlaneNotFound {
                id: source.dataplane_id,
            })?
            .allocation
            .mode();

        let bucket = self.archive_config().bucket.clone().ok_or_else(|| {
            // Not a not-found: the archive is recorded, this installation just
            // has nowhere to read it from, which is a deployment question and
            // not something the caller can fix by asking differently.
            CoreError::InternalError(
                "this installation has no archive bucket configured".to_string(),
            )
        })?;

        let planned = plan_restore(&archive, &source, source_mode, bucket.as_str())?;

        let recovery = DeploymentServiceImpl::new(
            deployment_repository,
            user_repository,
            aether_postgres::dataplane::PostgresDataPlaneRepository::new(&tx),
            organisation_repository,
            LocalClusterProvisioner,
            self.placement_windows(),
            AetherPolicy::new(permissions_in(&tx)),
        )
        .create_deployment(
            identity,
            CreateDeploymentCommand::new(
                command.organisation_id,
                command.name,
                planned.target.release.kind.clone(),
                planned.target.release.version.clone(),
                command.requested_by,
                // Both inherited. A restore is not the moment to change
                // product, size or environment: coming back as something else
                // is a migration, and a migration is a restore plus a decision.
                source.environment,
                command.region,
                planned.offer,
            )
            .recovering(Recovery {
                backup: archive.id,
                resources: planned.resources,
            }),
        )
        .await?;

        // The recovery archives on the platform's default terms from the
        // moment it exists, for the reason a created deployment does: the
        // alternative is an instance holding restored production data and
        // backed up by nothing.
        let archive_directive = match self
            .archive_config()
            .destination_for(recovery.organisation_id, recovery.id)
        {
            None => None,
            Some(destination) => {
                let schedule = BackupSchedule::default_for(
                    recovery.id,
                    recovery.organisation_id,
                    chrono::Utc::now(),
                );
                backup_schedule_repository.save(schedule.clone()).await?;

                Some(archive_section(
                    &destination,
                    self.archive_encryption(),
                    &schedule,
                ))
            }
        };

        let mut payload = deployment_payload(&recovery, archive_directive);
        payload["restore"] = restore_section(&planned, &archive);

        ActionServiceImpl::new(action_repository)
            .record_action(RecordActionCommand::new(
                recovery.id,
                recovery.dataplane_id,
                // Its own type, not `deployment.create`. A data plane that
                // treats the two alike would provision an empty instance and
                // report it running, and the difference between that and a
                // restore is the whole point.
                ActionType("deployment.restore".to_string()),
                ActionTarget {
                    kind: TargetKind::Deployment,
                    id: recovery.id.0,
                },
                ActionPayload { data: payload },
                ActionVersion(1),
                ActionSource::User {
                    user_id: command.requested_by.0,
                },
            ))
            .await?;

        Ok(recovery)
    }
}
