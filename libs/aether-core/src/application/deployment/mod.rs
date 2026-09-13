use aether_auth::Identity;
use aether_macros::transactional;
use chrono::Duration;
use serde_json::json;

use crate::{
    AetherService, CoreError,
    action::{
        ActionPayload, ActionSource, ActionTarget, ActionType, ActionVersion, TargetKind,
        commands::RecordActionCommand, ports::ActionService, service::ActionServiceImpl,
    },
    backups::{
        ArchiveDestination, BackupSchedule, StoreEncryption, ports::BackupScheduleRepository,
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
use crate::{infrastructure::role::permissions_in, policy::AetherPolicy};

/// The payload every `deployment.*` action carries.
///
/// One function rather than a literal at each call site, because Genesis
/// deserialises a single `DeploymentPayloadV1` before it looks at the routing
/// key: a delete built from a smaller subset failed on `missing field kind`
/// and was retried until somebody read the log. Two literals could drift
/// again; one cannot.
fn deployment_payload(
    deployment: &Deployment,
    archive: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut payload = json!({
        "deployment_id": deployment.id.0,
        "dataplane_id": deployment.dataplane_id.0,
        "organisation_id": deployment.organisation_id.0,
        "name": deployment.name.0.clone(),
        "kind": deployment.kind.to_string(),
        "version": deployment.version.to_string(),
        "namespace": deployment.namespace.clone(),
        "created_by": deployment.created_by.0,
        // The same numbers that reserved room on the data plane. Genesis used
        // to invent these, so what was reserved and what was deployed were
        // unrelated.
        "cpu_millis": deployment.resources.cpu_millis,
        "memory_mib": deployment.resources.memory_mib,
        "storage_gib": deployment.resources.storage_gib,
    });

    if let Some(archive) = archive {
        payload["archive"] = archive;
    }

    payload
}

/// Where this deployment archives, and when.
///
/// The control plane owns the layout, so the destination is computed here and
/// carried rather than rebuilt on the other side: a data plane deriving the
/// prefix itself would be a second implementation of the rule, and an archive
/// written under the wrong prefix is still an archive.
///
/// No method travels. The only one a data plane can carry out is a base backup
/// to the object store, and a field with one possible value reads like a
/// choice. It arrives when the second mechanism does.
fn archive_section(
    destination: &ArchiveDestination,
    encryption: &StoreEncryption,
    schedule: &BackupSchedule,
) -> serde_json::Value {
    json!({
        "destination_path": destination.as_url(),
        "encryption": encryption.as_archive_directive(),
        "schedule": {
            // Local to the zone beside it, never converted here. Converting to
            // UTC once, at write time, freezes the offset that applied that
            // day, and daylight saving moves it twice a year afterwards.
            "cron": schedule.to_cron(),
            "zone": schedule.zone.name(),
            "enabled": schedule.enabled,
        },
    })
}

impl DeploymentService for AetherService {
    #[transactional(deployment, user, data_plane, action, backup_schedule)]
    async fn create_deployment(
        &self,
        identity: Identity,
        command: CreateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        let deployment = DeploymentServiceImpl::new(
            deployment_repository,
            user_repository,
            data_plane_repository,
            LocalClusterProvisioner,
            self.placement_windows(),
            AetherPolicy::new(permissions_in(&tx)),
        )
        .create_deployment(identity, command)
        .await?;

        // A deployment starts backed up. The alternative is a platform where
        // the first thing anybody learns about backups is that they did not
        // have any -- and where the schedule exists only once somebody has
        // been to the settings screen, which is after the incident.
        //
        // Written only when there is somewhere to archive to. A schedule on an
        // installation with no bucket is a row promising something nothing
        // will carry out.
        let archive = match self
            .archive_config()
            .destination_for(deployment.organisation_id, deployment.id)
        {
            None => None,
            Some(destination) => {
                let schedule = BackupSchedule::default_for(
                    deployment.id,
                    deployment.organisation_id,
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
                    data: deployment_payload(&deployment, archive),
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
    async fn purge_deleted_deployments(&self, retention: Duration) -> Result<u64, CoreError> {
        DeploymentServiceImpl::new(
            deployment_repository,
            user_repository,
            data_plane_repository,
            LocalClusterProvisioner,
            self.placement_windows(),
            AetherPolicy::new(permissions_in(&tx)),
        )
        .purge_deleted_deployments(retention)
        .await
    }

    #[transactional(deployment, user, data_plane, action)]
    async fn delete_deployment(
        &self,
        deployment_id: DeploymentId,
    ) -> Result<Deployment, CoreError> {
        let deployment = DeploymentServiceImpl::new(
            deployment_repository,
            user_repository,
            data_plane_repository,
            LocalClusterProvisioner,
            self.placement_windows(),
            AetherPolicy::new(permissions_in(&tx)),
        )
        .delete_deployment(deployment_id)
        .await?;

        // Recorded in the same transaction as the soft delete, for the reason
        // the create path records one: an action that outlives a rolled-back
        // deletion would have Genesis tear down resources for a deployment the
        // control plane still considers live.
        //
        // Without this, deleting set `status = 'deleting'` and `deleted_at`,
        // published nothing, and left the row in `deleting` for ever with the
        // Kubernetes resources still running. Genesis has handled
        // `deployment.delete` since it was written; nothing ever sent one.
        ActionServiceImpl::new(action_repository)
            .record_action(RecordActionCommand::new(
                deployment.id,
                deployment.dataplane_id,
                ActionType("deployment.delete".to_string()),
                ActionTarget {
                    kind: TargetKind::Deployment,
                    id: deployment.id.0,
                },
                ActionPayload {
                    data: deployment_payload(&deployment, None),
                },
                ActionVersion(1),
                ActionSource::System,
            ))
            .await?;

        Ok(deployment)
    }

    #[transactional(deployment, user, data_plane, action)]
    async fn delete_deployment_for_organisation(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<Deployment, CoreError> {
        let deployment = DeploymentServiceImpl::new(
            deployment_repository,
            user_repository,
            data_plane_repository,
            LocalClusterProvisioner,
            self.placement_windows(),
            AetherPolicy::new(permissions_in(&tx)),
        )
        .delete_deployment_for_organisation(identity, organisation_id, deployment_id)
        .await?;

        // Recorded in the same transaction as the soft delete, for the reason
        // the create path records one: an action that outlives a rolled-back
        // deletion would have Genesis tear down resources for a deployment the
        // control plane still considers live.
        //
        // Without this, deleting set `status = 'deleting'` and `deleted_at`,
        // published nothing, and left the row in `deleting` for ever with the
        // Kubernetes resources still running. Genesis has handled
        // `deployment.delete` since it was written; nothing ever sent one.
        ActionServiceImpl::new(action_repository)
            .record_action(RecordActionCommand::new(
                deployment.id,
                deployment.dataplane_id,
                ActionType("deployment.delete".to_string()),
                ActionTarget {
                    kind: TargetKind::Deployment,
                    id: deployment.id.0,
                },
                ActionPayload {
                    data: deployment_payload(&deployment, None),
                },
                ActionVersion(1),
                ActionSource::System,
            ))
            .await?;

        Ok(deployment)
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
            LocalClusterProvisioner,
            self.placement_windows(),
            AetherPolicy::new(permissions_in(&tx)),
        )
        .get_deployment(deployment_id)
        .await
    }

    #[transactional(deployment, user, data_plane)]
    async fn get_deployment_for_organisation(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<Deployment, CoreError> {
        DeploymentServiceImpl::new(
            deployment_repository,
            user_repository,
            data_plane_repository,
            LocalClusterProvisioner,
            self.placement_windows(),
            AetherPolicy::new(permissions_in(&tx)),
        )
        .get_deployment_for_organisation(identity, organisation_id, deployment_id)
        .await
    }

    #[transactional(deployment, user, data_plane)]
    async fn list_deployments_by_organisation(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Deployment>, CoreError> {
        DeploymentServiceImpl::new(
            deployment_repository,
            user_repository,
            data_plane_repository,
            LocalClusterProvisioner,
            self.placement_windows(),
            AetherPolicy::new(permissions_in(&tx)),
        )
        .list_deployments_by_organisation(identity, organisation_id)
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
            LocalClusterProvisioner,
            self.placement_windows(),
            AetherPolicy::new(permissions_in(&tx)),
        )
        .update_deployment(deployment_id, command)
        .await
    }

    #[transactional(deployment, user, data_plane)]
    async fn update_deployment_for_organisation(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
        command: UpdateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        DeploymentServiceImpl::new(
            deployment_repository,
            user_repository,
            data_plane_repository,
            LocalClusterProvisioner,
            self.placement_windows(),
            AetherPolicy::new(permissions_in(&tx)),
        )
        .update_deployment_for_organisation(identity, organisation_id, deployment_id, command)
        .await
    }
}

#[cfg(test)]
mod tests {
    fn caller() -> Identity {
        Identity::User(aether_auth::User {
            id: uuid::Uuid::from_u128(9).to_string(),
            username: "somebody".to_string(),
            email: None,
            name: None,
            roles: Vec::new(),
        })
    }

    use super::*;
    use crate::ArchiveConfig;
    use crate::dataplane::value_objects::DeploymentResources;
    use crate::dataplane::value_objects::{DataPlaneMode, Region};
    use crate::domain::deployments::{DeploymentKind, DeploymentName, DeploymentStatus};
    use crate::domain::user::UserId;
    use sqlx::postgres::PgPoolOptions;
    use std::time::Duration;
    use uuid::Uuid;

    /// Genesis deserialises one `DeploymentPayloadV1` before it looks at the
    /// routing key, so every `deployment.*` action must carry the full set --
    /// including a delete, which needs none of it beyond the namespace.
    ///
    /// The field list is duplicated from `genesis-core`, deliberately and
    /// visibly: this crate does not depend on it, and a payload that fails to
    /// deserialise there costs a retry loop and a log dive rather than a
    /// compile error. Until the two share a type, this is the cheapest thing
    /// that fails on the right side.
    #[test]
    fn the_action_payload_carries_every_field_genesis_requires() {
        let deployment = sample_deployment();
        let payload = deployment_payload(&deployment, None);

        for field in [
            "deployment_id",
            "dataplane_id",
            "organisation_id",
            "name",
            "kind",
            "version",
            "namespace",
            "created_by",
        ] {
            assert!(
                payload.get(field).is_some_and(|v| !v.is_null()),
                "missing field `{field}` -- genesis rejects the whole payload"
            );
        }
    }

    /// The sizing travels too, so what placement reserved and what the operator
    /// deploys cannot disagree.
    #[test]
    fn the_action_payload_carries_the_resources_placement_reserved() {
        let deployment = sample_deployment();
        let payload = deployment_payload(&deployment, None);

        assert_eq!(payload["cpu_millis"], 500);
        assert_eq!(payload["memory_mib"], 1024);
        assert_eq!(payload["storage_gib"], 1);
    }

    /// The half of the chain that was missing: the operator has known how to
    /// archive since the CRDs landed, and nothing ever told it where to.
    #[test]
    fn the_action_payload_tells_the_data_plane_where_to_archive() {
        let deployment = sample_deployment();
        let destination = ArchiveConfig {
            bucket: Some(crate::backups::BucketName::new("aether-backups").unwrap()),
            encryption: StoreEncryption::Managed,
        }
        .destination_for(deployment.organisation_id, deployment.id)
        .expect("a bucket is configured");
        let schedule = BackupSchedule::default_for(
            deployment.id,
            deployment.organisation_id,
            chrono::Utc::now(),
        );

        let payload = deployment_payload(
            &deployment,
            Some(archive_section(
                &destination,
                &StoreEncryption::Managed,
                &schedule,
            )),
        );

        assert_eq!(
            payload["archive"]["destination_path"],
            json!(format!(
                "s3://aether-backups/{}/{}",
                deployment.organisation_id, deployment.id
            ))
        );
        assert_eq!(payload["archive"]["encryption"], json!("AES256"));
        assert_eq!(
            payload["archive"]["schedule"]["cron"],
            json!("0 30 2 * * *")
        );
        assert_eq!(payload["archive"]["schedule"]["zone"], json!("UTC"));
        assert_eq!(payload["archive"]["schedule"]["enabled"], json!(true));
    }

    /// An installation that archives nowhere says so by absence. A deployment
    /// carrying an empty destination would produce a cluster that looks
    /// configured and fails every archive, hours later.
    #[test]
    fn a_deployment_with_nowhere_to_archive_carries_no_destination() {
        let deployment = sample_deployment();

        assert!(
            ArchiveConfig::default()
                .destination_for(deployment.organisation_id, deployment.id)
                .is_none()
        );
        assert!(
            deployment_payload(&deployment, None)
                .get("archive")
                .is_none()
        );
    }

    /// The cron stays local and the zone travels beside it. Converting once,
    /// here, would freeze whichever offset applied on the day the deployment
    /// was created, and daylight saving moves it twice a year afterwards.
    #[test]
    fn the_schedule_travels_in_its_own_zone() {
        let deployment = sample_deployment();
        let mut schedule = BackupSchedule::default_for(
            deployment.id,
            deployment.organisation_id,
            chrono::Utc::now(),
        );
        schedule.zone = "Europe/Paris".parse().expect("a real zone");

        let section = archive_section(
            &ArchiveConfig {
                bucket: Some(crate::backups::BucketName::new("aether-backups").unwrap()),
                encryption: StoreEncryption::Managed,
            }
            .destination_for(deployment.organisation_id, deployment.id)
            .unwrap(),
            &StoreEncryption::Managed,
            &schedule,
        );

        assert_eq!(section["schedule"]["cron"], json!("0 30 2 * * *"));
        assert_eq!(section["schedule"]["zone"], json!("Europe/Paris"));
    }

    fn sample_deployment() -> Deployment {
        let at = chrono::Utc::now();
        Deployment {
            id: DeploymentId(uuid::Uuid::new_v4()),
            organisation_id: OrganisationId(uuid::Uuid::new_v4()),
            dataplane_id: aether_domain::dataplane::value_objects::DataPlaneId(uuid::Uuid::new_v4()),
            name: aether_domain::deployments::DeploymentName("auth".to_string()),
            kind: aether_domain::deployments::DeploymentKind::Ferriskey,
            version: aether_domain::version::Version::new(26, 0, 1),
            status: aether_domain::deployments::DeploymentStatus::Pending,
            namespace: "production-auth".to_string(),
            resources: aether_domain::dataplane::value_objects::DeploymentResources::DEFAULT,
            created_by: aether_domain::user::UserId(uuid::Uuid::new_v4()),
            created_at: at,
            updated_at: at,
            deployed_at: None,
            deleted_at: None,
            auto_upgrade: Default::default(),
            maintenance_window: None,
            network_access: aether_domain::deployments::network::NetworkAccess::Open,
        }
    }

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
            aether_domain::version::Version::new(1, 0, 0),
            DeploymentStatus::Pending,
            "namespace".to_string(),
            UserId(Uuid::new_v4()),
            Region::new("fr-par"),
            DataPlaneMode::Shared,
            DeploymentResources::DEFAULT,
        );

        let result = service().create_deployment(caller(), command).await;
        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn list_deployments_maps_pool_error() {
        let result = service()
            .list_deployments_by_organisation(caller(), OrganisationId(Uuid::new_v4()))
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
                caller(),
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
                caller(),
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
                caller(),
                OrganisationId(Uuid::new_v4()),
                DeploymentId(Uuid::new_v4()),
                command,
            )
            .await;

        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }
}
