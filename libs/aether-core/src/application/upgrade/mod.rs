use aether_auth::Identity;
use aether_domain::{
    CoreError,
    action::TargetKind,
    action::{
        ActionPayload, ActionSource, ActionTarget, ActionType, ActionVersion,
        commands::RecordActionCommand, ports::ActionService, service::ActionServiceImpl,
    },
    upgrades::{
        commands::RequestUpgradeCommand,
        ports::{AcceptedUpgrade, UpgradeService},
        service::UpgradeServiceImpl,
    },
};
use aether_macros::transactional;
use serde_json::json;

use crate::{
    AetherService,
    infrastructure::role::{PostgresRoleRepository, RolePermissionProvider},
    policy::AetherPolicy,
};

impl UpgradeService for AetherService {
    #[transactional(deployment, release, action)]
    async fn request_upgrade(
        &self,
        identity: Identity,
        command: RequestUpgradeCommand,
    ) -> Result<AcceptedUpgrade, CoreError> {
        let target = command.target.clone();

        // The provider reads roles through the surrounding transaction, so a
        // permission check cannot miss a role the same transaction wrote.
        let accepted = UpgradeServiceImpl::new(
            deployment_repository,
            release_repository,
            AetherPolicy::new(RolePermissionProvider::new(PostgresRoleRepository::new(
                &tx,
            ))),
        )
        .request_upgrade(identity, command)
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
                        "to_version": target.to_string(),
                    }),
                },
                ActionVersion(1),
                ActionSource::System,
            ))
            .await?;

        Ok(accepted)
    }
}
