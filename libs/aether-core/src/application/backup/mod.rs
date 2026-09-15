use aether_auth::Identity;
use aether_domain::dataplane::herald_identity::speaking_for;
use aether_domain::{
    CoreError,
    action::{
        ActionPayload, ActionSource, ActionTarget, ActionType, ActionVersion, TargetKind,
        commands::RecordActionCommand,
        ports::{ActionRepository, ActionService},
        service::ActionServiceImpl,
    },
    backups::{
        Backup, BackupSchedule,
        commands::{
            AskForBackupCommand, RecordArchiveCommand, RecordArchiveFailureCommand,
            SetBackupScheduleCommand,
        },
        plan_restore,
        ports::{BackupRepository, BackupScheduleRepository, BackupService},
        restore::{PlannedRestore, RestoreBackupCommand},
        schedule::refuse_if_one_is_already_coming,
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
    user::ports::UserRepository,
};
use aether_macros::transactional;

use crate::{
    AetherService,
    application::deployment::{archive_section, deployment_payload},
    infrastructure::{provisioner::LocalClusterProvisioner, role::permissions_in},
    policy::{AetherPolicy, PlatformRightsPolicy},
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
    #[transactional(backup, backup_schedule, deployment, audit, data_plane)]
    async fn record_archive(
        &self,
        identity: Identity,
        command: RecordArchiveCommand,
    ) -> Result<Backup, CoreError> {
        // Which data plane is reporting, read from its credential. It used to
        // be whatever the report said.
        let speaking = speaking_for(&data_plane_repository, &identity).await?;

        BackupServiceImpl::new(
            backup_repository,
            backup_schedule_repository,
            deployment_repository,
            audit_repository,
            AetherPolicy::new(permissions_in(&tx)),
            PlatformRightsPolicy::new(aether_postgres::platform::PostgresOperatorRepository::new(
                &tx,
            )),
        )
        .record_archive(speaking, command)
        .await
    }

    #[transactional(backup, backup_schedule, deployment, audit, data_plane)]
    async fn record_archive_failure(
        &self,
        identity: Identity,
        command: RecordArchiveFailureCommand,
    ) -> Result<(), CoreError> {
        let speaking = speaking_for(&data_plane_repository, &identity).await?;

        BackupServiceImpl::new(
            backup_repository,
            backup_schedule_repository,
            deployment_repository,
            audit_repository,
            AetherPolicy::new(permissions_in(&tx)),
            PlatformRightsPolicy::new(aether_postgres::platform::PostgresOperatorRepository::new(
                &tx,
            )),
        )
        .record_archive_failure(speaking, command)
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
            PlatformRightsPolicy::new(aether_postgres::platform::PostgresOperatorRepository::new(
                &tx,
            )),
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
            PlatformRightsPolicy::new(aether_postgres::platform::PostgresOperatorRepository::new(
                &tx,
            )),
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
            PlatformRightsPolicy::new(aether_postgres::platform::PostgresOperatorRepository::new(
                &tx,
            )),
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

    /// One transaction for the check and the action, because the check is
    /// about what has already been recorded: two requests landing together
    /// would each find no other and both record one.
    #[transactional(backup, backup_schedule, audit, action, user, platform_operator)]
    async fn ask_for_backup(
        &self,
        identity: Identity,
        command: AskForBackupCommand,
    ) -> Result<(), CoreError> {
        let asked_at = chrono::Utc::now();
        let deployment_id = command.deployment_id;
        let requested_by = command.requested_by;

        let deployment = BackupServiceImpl::new(
            aether_postgres::backups::PostgresBackupRepository::new(&tx),
            backup_schedule_repository,
            aether_postgres::deployments::PostgresDeploymentRepository::new(&tx),
            audit_repository,
            AetherPolicy::new(permissions_in(&tx)),
            PlatformRightsPolicy::new(platform_operator_repository),
        )
        .ask_for_backup(identity, command)
        .await?;

        // An installation that archives nowhere cannot take a backup, and an
        // action asking for one is work that cannot succeed. Refused here,
        // where somebody reads the answer, rather than on a data plane that
        // would report a failure nobody asked about.
        let destination = self
            .archive_config()
            .destination_for(deployment.organisation_id, deployment.id)
            .ok_or_else(|| {
                CoreError::InternalError("this installation has nowhere to archive to".to_string())
            })?;

        // The row's own id, not the subject the token carries. An action
        // records who asked as a foreign key into `users`, and a subject
        // written straight into that column is refused by the database --
        // which is how this was found, as an opaque 400 with nothing in the
        // logs.
        let asker = user_repository
            .find_by_sub(&requested_by.to_string())
            .await?
            .ok_or(CoreError::InvalidIdentity)?
            .id;

        let asked_for = ActionType("deployment.backup".to_string());

        refuse_if_one_is_already_coming(
            asked_at,
            action_repository
                .last_of_type(deployment_id, &asked_for)
                .await?,
            // Newest first, so the first row is the most recent archive this
            // deployment actually has.
            backup_repository
                .list_for_deployment(&deployment_id)
                .await?
                .first()
                // When it finished, not when it started: a request answered
                // by an archive that began before it is not answered at all.
                .map(|archive| archive.finished_at),
        )?;

        // The schedule travels with it, unchanged. The data plane is told
        // where to write and on what terms in one payload, so a one-off
        // archive cannot land somewhere the scheduled ones do not.
        let schedule =
            BackupSchedule::default_for(deployment.id, deployment.organisation_id, asked_at);
        let archive = archive_section(&destination, self.archive_encryption(), &schedule);

        ActionServiceImpl::new(action_repository)
            .record_action(RecordActionCommand::new(
                deployment.id,
                deployment.dataplane_id,
                asked_for,
                ActionTarget {
                    kind: TargetKind::Deployment,
                    id: deployment.id.0,
                },
                ActionPayload {
                    data: deployment_payload(&deployment, Some(archive)),
                },
                ActionVersion(1),
                ActionSource::User { user_id: asker.0 },
            ))
            .await?;

        Ok(())
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
            PlatformRightsPolicy::new(aether_postgres::platform::PostgresOperatorRepository::new(
                &tx,
            )),
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
