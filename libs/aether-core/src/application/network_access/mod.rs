use aether_auth::Identity;
use aether_domain::{
    CoreError,
    action::{
        ActionPayload, ActionSource, ActionTarget, ActionType, ActionVersion, TargetKind,
        commands::RecordActionCommand, ports::ActionService, service::ActionServiceImpl,
    },
    deployments::{
        Deployment, DeploymentId, network::NetworkAccess,
        network_service::NetworkAccessServiceImpl, ports::NetworkAccessService,
    },
    organisation::OrganisationId,
};
use aether_macros::transactional;
use serde_json::json;

use crate::{AetherService, infrastructure::role::permissions_in, policy::AetherPolicy};

/// What the data plane is told about a deployment's allow list.
///
/// The whole rule every time, never a difference. The edge applies a state
/// rather than replaying a history, and a data plane that missed one message
/// would otherwise be enforcing a rule nobody can name.
fn network_payload(deployment: &Deployment) -> serde_json::Value {
    json!({
        "deployment_id": deployment.id.0,
        "dataplane_id": deployment.dataplane_id.0,
        "organisation_id": deployment.organisation_id.0,
        "namespace": deployment.namespace,
        "kind": deployment.kind.to_string(),
        // Absent means open, which is the same shape the domain uses: there
        // is no empty list to confuse with it on this side either.
        "allowed_cidrs": match &deployment.network_access {
            NetworkAccess::Open => serde_json::Value::Null,
            NetworkAccess::Restricted { allowed } => json!(
                allowed
                    .ranges()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            ),
        },
    })
}

impl NetworkAccessService for AetherService {
    #[transactional(deployment)]
    async fn network_access(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<NetworkAccess, CoreError> {
        NetworkAccessServiceImpl::new(
            deployment_repository,
            AetherPolicy::new(permissions_in(&tx)),
        )
        .network_access(identity, organisation_id, deployment_id)
        .await
    }

    #[transactional(deployment, action)]
    async fn set_network_access(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
        access: NetworkAccess,
    ) -> Result<Deployment, CoreError> {
        let deployment = NetworkAccessServiceImpl::new(
            deployment_repository,
            AetherPolicy::new(permissions_in(&tx)),
        )
        .set_network_access(identity, organisation_id, deployment_id, access)
        .await?;

        // In the same transaction as the write it describes, for the reason
        // the upgrade path records its action in one: an action that outlives
        // a rolled-back change would have the edge enforce a rule the control
        // plane never accepted.
        ActionServiceImpl::new(action_repository)
            .record_action(RecordActionCommand::new(
                deployment.id,
                deployment.dataplane_id,
                ActionType("deployment.network_access".to_string()),
                ActionTarget {
                    kind: TargetKind::Deployment,
                    id: deployment.id.0,
                },
                ActionPayload {
                    data: network_payload(&deployment),
                },
                ActionVersion(1),
                ActionSource::System,
            ))
            .await?;

        Ok(deployment)
    }
}
