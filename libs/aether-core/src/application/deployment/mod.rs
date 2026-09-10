use aether_macros::transactional;
use serde_json::json;

use crate::{
    AetherService, CoreError,
    action::{
        ActionPayload, ActionSource, ActionTarget, ActionType, ActionVersion, TargetKind,
        commands::RecordActionCommand, ports::ActionService, service::ActionServiceImpl,
    },
    deployments::{
        Deployment, DeploymentId,
        commands::{CreateDeploymentCommand, UpdateDeploymentCommand},
        ports::DeploymentService,
        service::DeploymentServiceImpl,
    },
    infrastructure::provisioner::LocalClusterProvisioner,
    organisation::OrganisationId,
};

impl DeploymentService for AetherService {
    #[transactional(deployment, user, data_plane, action)]
    async fn create_deployment(
        &self,
        command: CreateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        let deployment = DeploymentServiceImpl::new(
            deployment_repository,
            user_repository,
            data_plane_repository,
            LocalClusterProvisioner::default(),
            self.heartbeat_window(),
        )
        .create_deployment(command)
        .await?;

        // Recorded in the same transaction as the insert: an action that
        // outlives a rolled-back deployment would have Herald publish work for
        // a deployment that does not exist.
        ActionServiceImpl::new(action_repository)
            .record_action(RecordActionCommand::new(
                deployment.id,
                deployment.dataplane_id,
                ActionType("deployment.create".to_string()),
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
                        "version": deployment.version.0.clone(),
                        "namespace": deployment.namespace.clone(),
                        "created_by": deployment.created_by.0,
                        // The same numbers that reserved room on the data
                        // plane. Genesis used to invent these, so what was
                        // reserved and what was deployed were unrelated.
                        "cpu_millis": deployment.resources.cpu_millis,
                        "memory_mib": deployment.resources.memory_mib,
                        "storage_gib": deployment.resources.storage_gib,
                    }),
                },
                ActionVersion(1),
                ActionSource::User {
                    user_id: deployment.created_by.0,
                },
            ))
            .await?;

        Ok(deployment)
    }

    #[transactional(deployment, user, data_plane)]
    async fn delete_deployment(&self, deployment_id: DeploymentId) -> Result<(), CoreError> {
        DeploymentServiceImpl::new(
            deployment_repository,
            user_repository,
            data_plane_repository,
            LocalClusterProvisioner::default(),
            self.heartbeat_window(),
        )
        .delete_deployment(deployment_id)
        .await
    }

    #[transactional(deployment, user, data_plane)]
    async fn delete_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<(), CoreError> {
        DeploymentServiceImpl::new(
            deployment_repository,
            user_repository,
            data_plane_repository,
            LocalClusterProvisioner::default(),
            self.heartbeat_window(),
        )
        .delete_deployment_for_organisation(organisation_id, deployment_id)
        .await
    }

    #[transactional(deployment, user, data_plane)]
    async fn get_deployment(
        &self,
        deployment_id: DeploymentId,
    ) -> Result<Option<Deployment>, CoreError> {
        DeploymentServiceImpl::new(
            deployment_repository,
            user_repository,
            data_plane_repository,
            LocalClusterProvisioner::default(),
            self.heartbeat_window(),
        )
        .get_deployment(deployment_id)
        .await
    }

    #[transactional(deployment, user, data_plane)]
    async fn get_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<Deployment, CoreError> {
        DeploymentServiceImpl::new(
            deployment_repository,
            user_repository,
            data_plane_repository,
            LocalClusterProvisioner::default(),
            self.heartbeat_window(),
        )
        .get_deployment_for_organisation(organisation_id, deployment_id)
        .await
    }

    #[transactional(deployment, user, data_plane)]
    async fn list_deployments_by_organisation(
        &self,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Deployment>, CoreError> {
        DeploymentServiceImpl::new(
            deployment_repository,
            user_repository,
            data_plane_repository,
            LocalClusterProvisioner::default(),
            self.heartbeat_window(),
        )
        .list_deployments_by_organisation(organisation_id)
        .await
    }

    #[transactional(deployment, user, data_plane)]
    async fn update_deployment(
        &self,
        deployment_id: DeploymentId,
        command: UpdateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        DeploymentServiceImpl::new(
            deployment_repository,
            user_repository,
            data_plane_repository,
            LocalClusterProvisioner::default(),
            self.heartbeat_window(),
        )
        .update_deployment(deployment_id, command)
        .await
    }

    #[transactional(deployment, user, data_plane)]
    async fn update_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
        command: UpdateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        DeploymentServiceImpl::new(
            deployment_repository,
            user_repository,
            data_plane_repository,
            LocalClusterProvisioner::default(),
            self.heartbeat_window(),
        )
        .update_deployment_for_organisation(organisation_id, deployment_id, command)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataplane::value_objects::DeploymentResources;
    use crate::dataplane::value_objects::{DataPlaneMode, Region};
    use crate::domain::deployments::{
        DeploymentKind, DeploymentName, DeploymentStatus, DeploymentVersion,
    };
    use crate::domain::user::UserId;
    use sqlx::postgres::PgPoolOptions;
    use std::time::Duration;
    use uuid::Uuid;

    fn service() -> AetherService {
        let pool = PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(50))
            .connect_lazy("postgres://user:pass@127.0.0.1:1/db")
            .expect("valid database url");
        AetherService::new(pool)
    }

    #[tokio::test]
    async fn create_deployment_maps_pool_error() {
        let command = CreateDeploymentCommand::new(
            OrganisationId(Uuid::new_v4()),
            DeploymentName("deployment".to_string()),
            DeploymentKind::Keycloak,
            DeploymentVersion("1.0.0".to_string()),
            DeploymentStatus::Pending,
            "namespace".to_string(),
            UserId(Uuid::new_v4()),
            Region::new("fr-par"),
            DataPlaneMode::Shared,
            DeploymentResources::DEFAULT,
        );

        let result = service().create_deployment(command).await;
        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn list_deployments_maps_pool_error() {
        let result = service()
            .list_deployments_by_organisation(OrganisationId(Uuid::new_v4()))
            .await;

        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn get_deployment_maps_pool_error() {
        let result = service().get_deployment(DeploymentId(Uuid::new_v4())).await;

        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn get_deployment_for_organisation_maps_pool_error() {
        let result = service()
            .get_deployment_for_organisation(
                OrganisationId(Uuid::new_v4()),
                DeploymentId(Uuid::new_v4()),
            )
            .await;

        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn delete_deployment_maps_pool_error() {
        let result = service()
            .delete_deployment(DeploymentId(Uuid::new_v4()))
            .await;

        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn delete_deployment_for_organisation_maps_pool_error() {
        let result = service()
            .delete_deployment_for_organisation(
                OrganisationId(Uuid::new_v4()),
                DeploymentId(Uuid::new_v4()),
            )
            .await;

        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn update_deployment_maps_pool_error() {
        let command = UpdateDeploymentCommand::new()
            .with_name(DeploymentName("name".to_string()))
            .with_status(DeploymentStatus::Pending);

        let result = service()
            .update_deployment(DeploymentId(Uuid::new_v4()), command)
            .await;

        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn update_deployment_for_organisation_maps_pool_error() {
        let command = UpdateDeploymentCommand::new()
            .with_name(DeploymentName("name".to_string()))
            .with_status(DeploymentStatus::Pending);

        let result = service()
            .update_deployment_for_organisation(
                OrganisationId(Uuid::new_v4()),
                DeploymentId(Uuid::new_v4()),
                command,
            )
            .await;

        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }
}
