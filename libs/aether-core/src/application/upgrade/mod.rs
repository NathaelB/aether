use aether_auth::Identity;
use aether_domain::{
    CoreError,
    action::TargetKind,
    action::{
        ActionPayload, ActionSource, ActionTarget, ActionType, ActionVersion,
        commands::RecordActionCommand, ports::ActionService, service::ActionServiceImpl,
    },
    audit::{
        AuditActor, AuditTarget, AuditTargetKind,
        commands::RecordAuditEntryCommand,
        ports::AuditService,
        service::{AuditServiceImpl, audit_actor},
    },
    deployments::Deployment,
    upgrades::{
        commands::{RequestUpgradeCommand, SetUpgradeSettingsCommand},
        ports::{AcceptedUpgrade, UpgradeProgress, UpgradeService},
        run::{InFlightUpgrade, UpgradeRun, UpgradeRunId, UpgradeTrigger},
        run_ports::UpgradeRunRepository,
        service::{ConfigurationChange, UpgradeServiceImpl},
    },
    user::ports::UserRepository,
};
use aether_macros::transactional;
use serde_json::json;

use crate::{
    AetherService,
    infrastructure::role::{PostgresRoleRepository, RolePermissionProvider},
    policy::AetherPolicy,
};

impl UpgradeService for AetherService {
    #[transactional(deployment, release, upgrade_run)]
    async fn upgrade_in_flight(
        &self,
        organisation_id: aether_domain::organisation::OrganisationId,
        deployment_id: aether_domain::deployments::DeploymentId,
    ) -> Result<Option<InFlightUpgrade>, CoreError> {
        UpgradeServiceImpl::new(
            deployment_repository,
            release_repository,
            upgrade_run_repository,
            AetherPolicy::new(RolePermissionProvider::new(PostgresRoleRepository::new(
                &tx,
            ))),
        )
        .upgrade_in_flight(organisation_id, deployment_id)
        .await
    }

    #[transactional(deployment, release, upgrade_run)]
    async fn advance_upgrade(
        &self,
        deployment_id: aether_domain::deployments::DeploymentId,
    ) -> Result<UpgradeProgress, CoreError> {
        UpgradeServiceImpl::new(
            deployment_repository,
            release_repository,
            upgrade_run_repository,
            AetherPolicy::new(RolePermissionProvider::new(PostgresRoleRepository::new(
                &tx,
            ))),
        )
        .advance_upgrade(deployment_id)
        .await
    }

    #[transactional(deployment, release, upgrade_run, user, audit)]
    async fn set_upgrade_settings(
        &self,
        identity: Identity,
        command: SetUpgradeSettingsCommand,
    ) -> Result<Deployment, CoreError> {
        // Resolved before the settings move, so an identity the platform
        // cannot name fails without having changed anything.
        let actor = audit_actor(&identity, &user_repository).await?;

        let applied = UpgradeServiceImpl::new(
            deployment_repository,
            release_repository,
            upgrade_run_repository,
            AetherPolicy::new(RolePermissionProvider::new(PostgresRoleRepository::new(
                &tx,
            ))),
        )
        .apply_upgrade_settings(identity, command)
        .await?;

        // In the same transaction as the write it describes, for the reason
        // the upgrade path records its action in one: an entry that outlives a
        // rolled-back change is a statement about something that never
        // happened, made by the one record nobody may correct afterwards.
        let audit = AuditServiceImpl::new(
            audit_repository,
            RolePermissionProvider::new(PostgresRoleRepository::new(&tx)),
        );

        for ConfigurationChange { action, change } in applied.configuration_changes()? {
            audit
                .record(
                    RecordAuditEntryCommand::new(
                        applied.after.organisation_id,
                        actor.clone(),
                        action,
                        AuditTarget {
                            kind: AuditTargetKind::Deployment,
                            id: applied.after.id.0,
                        },
                    )
                    .with_change(change),
                )
                .await?;
        }

        Ok(applied.after)
    }

    #[transactional(deployment, release, action, upgrade_run, user, audit)]
    async fn request_upgrade(
        &self,
        identity: Identity,
        command: RequestUpgradeCommand,
    ) -> Result<AcceptedUpgrade, CoreError> {
        // Resolved before the upgrade is accepted, so an identity the platform
        // cannot name fails without having moved anything. A run that cannot
        // say who asked is the one thing this record exists to prevent.
        let asked_by = user_repository
            .find_by_sub(identity.id())
            .await?
            .ok_or(CoreError::InvalidIdentity)?;

        // The provider reads roles through the surrounding transaction, so a
        // permission check cannot miss a role the same transaction wrote.
        let accepted = UpgradeServiceImpl::new(
            deployment_repository,
            release_repository,
            aether_postgres::upgrades::PostgresUpgradeRunRepository::new(&tx),
            AetherPolicy::new(RolePermissionProvider::new(PostgresRoleRepository::new(
                &tx,
            ))),
        )
        .request_upgrade(identity, command)
        .await?;

        // The run is the history the deployment row cannot keep: its status
        // and version hold one attempt at a time, so the moment a second
        // upgrade starts the first stops being visible anywhere else.
        upgrade_run_repository
            .insert(UpgradeRun::start(
                UpgradeRunId(uuid::Uuid::new_v4()),
                &accepted,
                UpgradeTrigger::Manual { by: asked_by.id },
                chrono::Utc::now(),
            ))
            .await?;

        // Recorded in the same transaction as the status change, for the
        // reason the create and delete paths record one: an action that
        // outlives a rolled-back upgrade would have Genesis move a version the
        // control plane never accepted.
        ActionServiceImpl::new(action_repository)
            .record_action(RecordActionCommand::new(
                accepted.deployment.id,
                accepted.deployment.dataplane_id,
                ActionType("deployment.upgrade".to_string()),
                ActionTarget {
                    kind: TargetKind::Deployment,
                    id: accepted.deployment.id.0,
                },
                ActionPayload {
                    data: json!({
                        "deployment_id": accepted.deployment.id.0,
                        "dataplane_id": accepted.deployment.dataplane_id.0,
                        "organisation_id": accepted.deployment.organisation_id.0,
                        "name": accepted.deployment.name.0.clone(),
                        "kind": accepted.deployment.kind.to_string(),
                        "namespace": accepted.deployment.namespace.clone(),
                        // Both ends travel. The data plane needs the target to
                        // apply, and the current version so a report about an
                        // upgrade that already happened can be told apart from
                        // one about the upgrade being asked for now.
                        "from_version": accepted.deployment.version.to_string(),
                        // The first step, not the target. A deployment several
                        // releases behind passes through the ones in between,
                        // and handing the data plane the far end would skip
                        // every migration on the way.
                        "to_version": accepted.first_step().to_string(),
                    }),
                },
                ActionVersion(1),
                ActionSource::System,
            ))
            .await?;

        // The action above says what the data plane was told to do; this says
        // who decided it. They answer different questions, and the second one
        // survives the action being pruned.
        let approved = ConfigurationChange::upgrade_approved(&accepted)?;
        AuditServiceImpl::new(
            audit_repository,
            RolePermissionProvider::new(PostgresRoleRepository::new(&tx)),
        )
        .record(
            RecordAuditEntryCommand::new(
                accepted.deployment.organisation_id,
                AuditActor::User {
                    user_id: asked_by.id.0,
                },
                approved.action,
                AuditTarget {
                    kind: AuditTargetKind::Deployment,
                    id: accepted.deployment.id.0,
                },
            )
            .with_change(approved.change),
        )
        .await?;

        Ok(accepted)
    }
}
