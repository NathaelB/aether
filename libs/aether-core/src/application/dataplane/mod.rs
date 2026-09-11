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
        ports::DataPlaneService,
        service::DataPlaneServiceImpl,
        value_objects::{CreateDataplaneCommand, DataPlaneId, ListDataPlaneDeploymentsCommand},
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
};
use serde_json::json;

use crate::{AetherService, infrastructure::role::permissions_in, policy::AetherPolicy};

impl DataPlaneService for AetherService {
    #[transactional(data_plane, deployment)]
    async fn create_dataplane(
        &self,
        identity: Identity,
        command: CreateDataplaneCommand,
    ) -> Result<DataPlane, CoreError> {
        DataPlaneServiceImpl::new(
            data_plane_repository,
            deployment_repository,
            self.heartbeat_window(),
        )
        .create_dataplane(identity, command)
        .await
    }

    #[transactional(data_plane, deployment)]
    async fn list_dataplanes(&self, identity: Identity) -> Result<Vec<DataPlane>, CoreError> {
        DataPlaneServiceImpl::new(
            data_plane_repository,
            deployment_repository,
            self.heartbeat_window(),
        )
        .list_dataplanes(identity)
        .await
    }

    #[transactional(data_plane, deployment)]
    async fn get_dataplane(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
    ) -> Result<DataPlane, CoreError> {
        DataPlaneServiceImpl::new(
            data_plane_repository,
            deployment_repository,
            self.heartbeat_window(),
        )
        .get_dataplane(identity, dataplane_id)
        .await
    }

    #[transactional(data_plane, deployment)]
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
        )
        .get_deployments_in_dataplane(identity, dataplane_id, command)
        .await
    }

    #[transactional(data_plane, deployment)]
    async fn list_regions(&self, identity: Identity) -> Result<Vec<Region>, CoreError> {
        DataPlaneServiceImpl::new(
            data_plane_repository,
            deployment_repository,
            self.heartbeat_window(),
        )
        .list_regions(identity)
        .await
    }

    #[transactional(data_plane, deployment, upgrade_run, action)]
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

    #[transactional(data_plane, deployment)]
    async fn record_heartbeat(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
        operator_version: Option<aether_domain::version::Version>,
    ) -> Result<bool, CoreError> {
        DataPlaneServiceImpl::new(
            data_plane_repository,
            deployment_repository,
            self.heartbeat_window(),
        )
        .record_heartbeat(identity, dataplane_id, operator_version)
        .await
    }
}
