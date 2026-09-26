use aether_auth::Identity;
use aether_domain::{
    CoreError,
    action::{
        ActionPayload, ActionSource, ActionTarget, ActionType, ActionVersion, TargetKind,
        commands::RecordActionCommand, ports::ActionService, service::ActionServiceImpl,
    },
    dataplane::value_objects::Region,
    dataplane::{
        entities::DataPlane,
        herald_identity::RegisteredDataPlane,
        ports::DataPlaneService,
        service::DataPlaneServiceImpl,
        value_objects::{
            CreateDataplaneCommand, DataPlaneId, ListDataPlaneDeploymentsCommand, ServiceIntent,
        },
    },
    deployments::Deployment,
    deployments::commands::ReportDeploymentOutcomeCommand,
    upgrades::{
        ports::{UpgradeProgress, UpgradeService},
        service::UpgradeServiceImpl,
    },
};
use aether_macros::transactional;
use aether_postgres::{
    catalog::PostgresReleaseRepository, deployments::PostgresDeploymentRepository,
    organisation::PostgresOrganisationRepository,
};
use serde_json::json;

use crate::{
    AetherService,
    infrastructure::role::permissions_in,
    policy::{AetherPolicy, PlatformRightsPolicy},
};

impl DataPlaneService for AetherService {
    #[transactional(data_plane, deployment, fleet_audit)]
    async fn create_dataplane(
        &self,
        identity: Identity,
        command: CreateDataplaneCommand,
    ) -> Result<RegisteredDataPlane, CoreError> {
        DataPlaneServiceImpl::new(
            data_plane_repository,
            deployment_repository,
            self.heartbeat_window(),
            PlatformRightsPolicy::new(aether_postgres::platform::PostgresOperatorRepository::new(
                &tx,
            )),
            self.herald_identities(),
            fleet_audit_repository,
        )
        .create_dataplane(identity, command)
        .await
    }

    /// Minting a client is a call to the identity provider, made inside this
    /// transaction. A failure there rolls the registration back, which is the
    /// answer that leaves the least behind: a data plane nothing can speak for
    /// is worse than one that was never registered.
    #[transactional(data_plane, deployment, fleet_audit)]
    async fn reissue_herald_credential(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
    ) -> Result<RegisteredDataPlane, CoreError> {
        DataPlaneServiceImpl::new(
            data_plane_repository,
            deployment_repository,
            self.heartbeat_window(),
            PlatformRightsPolicy::new(aether_postgres::platform::PostgresOperatorRepository::new(
                &tx,
            )),
            self.herald_identities(),
            fleet_audit_repository,
        )
        .reissue_herald_credential(identity, dataplane_id)
        .await
    }

    #[transactional(data_plane, deployment, fleet_audit)]
    async fn list_dataplanes(&self, identity: Identity) -> Result<Vec<DataPlane>, CoreError> {
        DataPlaneServiceImpl::new(
            data_plane_repository,
            deployment_repository,
            self.heartbeat_window(),
            PlatformRightsPolicy::new(aether_postgres::platform::PostgresOperatorRepository::new(
                &tx,
            )),
            self.herald_identities(),
            fleet_audit_repository,
        )
        .list_dataplanes(identity)
        .await
    }

    #[transactional(data_plane, deployment, fleet_audit)]
    async fn list_all_dataplanes(&self) -> Result<Vec<DataPlane>, CoreError> {
        DataPlaneServiceImpl::new(
            data_plane_repository,
            deployment_repository,
            self.heartbeat_window(),
            PlatformRightsPolicy::new(aether_postgres::platform::PostgresOperatorRepository::new(
                &tx,
            )),
            self.herald_identities(),
            fleet_audit_repository,
        )
        .list_all_dataplanes()
        .await
    }

    #[transactional(data_plane, deployment, fleet_audit)]
    async fn get_dataplane(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
    ) -> Result<DataPlane, CoreError> {
        DataPlaneServiceImpl::new(
            data_plane_repository,
            deployment_repository,
            self.heartbeat_window(),
            PlatformRightsPolicy::new(aether_postgres::platform::PostgresOperatorRepository::new(
                &tx,
            )),
            self.herald_identities(),
            fleet_audit_repository,
        )
        .get_dataplane(identity, dataplane_id)
        .await
    }

    #[transactional(data_plane, deployment, fleet_audit)]
    async fn get_deployments_in_dataplane(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
        command: ListDataPlaneDeploymentsCommand,
    ) -> Result<Vec<Deployment>, CoreError> {
        DataPlaneServiceImpl::new(
            data_plane_repository,
            deployment_repository,
            self.heartbeat_window(),
            PlatformRightsPolicy::new(aether_postgres::platform::PostgresOperatorRepository::new(
                &tx,
            )),
            self.herald_identities(),
            fleet_audit_repository,
        )
        .get_deployments_in_dataplane(identity, dataplane_id, command)
        .await
    }

    #[transactional(data_plane, deployment, fleet_audit)]
    async fn list_regions(&self, identity: Identity) -> Result<Vec<Region>, CoreError> {
        DataPlaneServiceImpl::new(
            data_plane_repository,
            deployment_repository,
            self.heartbeat_window(),
            PlatformRightsPolicy::new(aether_postgres::platform::PostgresOperatorRepository::new(
                &tx,
            )),
            self.herald_identities(),
            fleet_audit_repository,
        )
        .list_regions(identity)
        .await
    }

    #[transactional(data_plane, deployment, fleet_audit, upgrade_run, action)]
    async fn report_outcome(
        &self,
        identity: Identity,
        command: ReportDeploymentOutcomeCommand,
    ) -> Result<bool, CoreError> {
        let deployment_id = command.deployment_id;

        let changed = DataPlaneServiceImpl::new(
            data_plane_repository,
            deployment_repository,
            self.heartbeat_window(),
            PlatformRightsPolicy::new(aether_postgres::platform::PostgresOperatorRepository::new(
                &tx,
            )),
            self.herald_identities(),
            fleet_audit_repository,
        )
        .report_outcome(identity, command)
        .await?;

        // A report that changed nothing says nothing about an upgrade either.
        if !changed {
            return Ok(false);
        }

        // An upgrade of several versions moves one step at a time, and this is
        // where the next one starts: only once the data plane has reported the
        // last one running, which after the operator's health check means it
        // is actually serving.
        let progress = UpgradeServiceImpl::new(
            PostgresDeploymentRepository::new(&tx),
            PostgresReleaseRepository::new(&tx),
            upgrade_run_repository,
            PostgresOrganisationRepository::new(&tx),
            AetherPolicy::new(permissions_in(&tx)),
        )
        .advance_upgrade(deployment_id)
        .await?;

        if let UpgradeProgress::NextStep { deployment, to } = progress {
            // Same transaction as the status change, for the reason the first
            // step records one: an action that outlives a rolled-back write
            // would have Genesis apply a version the control plane never
            // committed to.
            ActionServiceImpl::new(action_repository)
                .record_action(RecordActionCommand::new(
                    deployment.id,
                    deployment.dataplane_id,
                    ActionType("deployment.upgrade".to_string()),
                    ActionTarget {
                        kind: TargetKind::Deployment,
                        id: deployment.id.0,
                    },
                    ActionPayload {
                        data: json!({
                            "deployment_id": deployment.id.0,
                            "dataplane_id": deployment.dataplane_id.0,
                            "organisation_id": deployment.organisation_id.0,
                            "name": deployment.name.0.clone(),
                            "kind": deployment.kind.to_string(),
                            "namespace": deployment.namespace.clone(),
                            "from_version": deployment.version.to_string(),
                            "to_version": to.to_string(),
                        }),
                    },
                    ActionVersion(1),
                    ActionSource::System,
                ))
                .await?;
        }

        Ok(changed)
    }

    #[transactional(data_plane, deployment, fleet_audit, platform_operator)]
    async fn set_dataplane_service(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
        service: ServiceIntent,
    ) -> Result<DataPlane, CoreError> {
        DataPlaneServiceImpl::new(
            data_plane_repository,
            deployment_repository,
            self.heartbeat_window(),
            PlatformRightsPolicy::new(platform_operator_repository),
            self.herald_identities(),
            fleet_audit_repository,
        )
        .set_dataplane_service(identity, dataplane_id, service)
        .await
    }

    #[transactional(data_plane, deployment, fleet_audit)]
    async fn record_heartbeat(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
        operator_version: Option<aether_domain::version::Version>,
        gateway_address: Option<String>,
    ) -> Result<bool, CoreError> {
        DataPlaneServiceImpl::new(
            data_plane_repository,
            deployment_repository,
            self.heartbeat_window(),
            PlatformRightsPolicy::new(aether_postgres::platform::PostgresOperatorRepository::new(
                &tx,
            )),
            self.herald_identities(),
            fleet_audit_repository,
        )
        .record_heartbeat(identity, dataplane_id, operator_version, gateway_address)
        .await
    }
}
