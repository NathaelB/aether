use aether_auth::Identity;
use chrono::{Duration, Utc};

use crate::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        ports::{DataPlaneRepository, DataPlaneService},
        value_objects::{
            CreateDataplaneCommand, DataPlaneId, DataPlaneStatus, ListDataPlaneDeploymentsCommand,
            Region,
        },
    },
    deployments::{
        Deployment, DeploymentId,
        commands::{DeploymentOutcome, ReportDeploymentOutcomeCommand},
        ports::DeploymentRepository,
    },
};
use uuid::Uuid;

#[derive(Debug)]
pub struct DataPlaneServiceImpl<DP, D>
where
    DP: DataPlaneRepository,
    D: DeploymentRepository,
{
    dataplane_repository: DP,
    deployment_repository: D,
    heartbeat_window: Duration,
}

impl<DP, D> DataPlaneServiceImpl<DP, D>
where
    DP: DataPlaneRepository,
    D: DeploymentRepository,
{
    pub fn new(
        dataplane_repository: DP,
        deployment_repository: D,
        heartbeat_window: Duration,
    ) -> Self {
        Self {
            dataplane_repository,
            deployment_repository,
            heartbeat_window,
        }
    }

    pub fn heartbeat_window(&self) -> Duration {
        self.heartbeat_window
    }
}

impl<DP, D> DataPlaneService for DataPlaneServiceImpl<DP, D>
where
    DP: DataPlaneRepository,
    D: DeploymentRepository,
{
    async fn create_dataplane(
        &self,
        identity: Identity,
        command: CreateDataplaneCommand,
    ) -> Result<DataPlane, CoreError> {
        if !identity.is_operator() {
            return Err(CoreError::PermissionDenied {
                reason: "data planes are operated, not consumed".to_string(),
            });
        }

        let dataplane = DataPlane::new(command.allocation, command.region, command.capacity);
        self.dataplane_repository.save(&dataplane).await?;

        Ok(dataplane)
    }

    async fn list_dataplanes(&self, identity: Identity) -> Result<Vec<DataPlane>, CoreError> {
        if !identity.is_operator() {
            return Err(CoreError::PermissionDenied {
                reason: "data planes are operated, not consumed".to_string(),
            });
        }

        self.dataplane_repository.list_all().await
    }

    async fn get_dataplane(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
    ) -> Result<DataPlane, CoreError> {
        if !identity.is_operator() {
            return Err(CoreError::PermissionDenied {
                reason: "data planes are operated, not consumed".to_string(),
            });
        }

        let dataplane = self
            .dataplane_repository
            .find_by_id(&dataplane_id)
            .await?
            .ok_or(CoreError::DataPlaneNotFound { id: dataplane_id })?;

        Ok(dataplane)
    }

    async fn get_deployments_in_dataplane(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
        command: ListDataPlaneDeploymentsCommand,
    ) -> Result<Vec<Deployment>, CoreError> {
        // Herald reads this for the data plane it serves; an operator reads it
        // to see what is placed where. A customer has no business here.
        if !identity.is_operator() && !identity.username().contains("herald-service") {
            return Err(CoreError::PermissionDenied {
                reason: "data planes are operated, not consumed".to_string(),
            });
        }

        let mut deployments = self
            .deployment_repository
            .list_by_dataplane(&dataplane_id)
            .await?;

        deployments.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| b.id.0.cmp(&a.id.0))
        });

        let mut shard_deployments: Vec<Deployment> = deployments
            .into_iter()
            .filter(|deployment| {
                (deployment.id.0.as_u128() % command.shard_count as u128) as usize
                    == command.shard_index
            })
            .collect();

        if let Some(cursor) = command.cursor {
            let cursor_id = Uuid::parse_str(&cursor)
                .map(DeploymentId)
                .map_err(|_| CoreError::InternalError("Invalid cursor format".to_string()))?;
            let offset = shard_deployments
                .iter()
                .position(|deployment| deployment.id == cursor_id)
                .map(|index| index + 1)
                .unwrap_or(shard_deployments.len());
            shard_deployments = shard_deployments.into_iter().skip(offset).collect();
        }

        Ok(shard_deployments.into_iter().take(command.limit).collect())
    }

    async fn list_regions(&self, _identity: Identity) -> Result<Vec<Region>, CoreError> {
        let dataplanes = self.dataplane_repository.list_all().await?;

        let mut regions: Vec<Region> = dataplanes
            .into_iter()
            .filter(|dataplane| {
                !matches!(
                    dataplane.status,
                    DataPlaneStatus::Disabled | DataPlaneStatus::Failed
                )
            })
            .map(|dataplane| dataplane.region)
            .collect();

        regions.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        regions.dedup();

        Ok(regions)
    }

    async fn report_outcome(
        &self,
        identity: Identity,
        command: ReportDeploymentOutcomeCommand,
    ) -> Result<bool, CoreError> {
        // Same rule as claim, ack and heartbeat: only Herald speaks for a data
        // plane. A caller able to forge an outcome could mark a live
        // deployment deleted.
        let client_id = identity.username();
        if !client_id.contains("herald-service") {
            return Err(CoreError::PermissionDenied {
                reason: "only herald can report a deployment outcome".to_string(),
            });
        }

        let Some(mut deployment) = self
            .deployment_repository
            .get_by_id(command.deployment_id)
            .await?
        else {
            // Unknown rather than failed: a report can outlive the row it
            // describes, and answering with an error would have the data plane
            // retry for ever.
            return Ok(false);
        };

        // A data plane may only report about what runs on it. Without this a
        // compromised or misconfigured Herald could mark another data plane's
        // deployments deleted.
        if deployment.dataplane_id != command.dataplane_id {
            return Err(CoreError::PermissionDenied {
                reason: "that deployment does not run on this data plane".to_string(),
            });
        }

        let now = Utc::now();
        let changed = match command.outcome {
            DeploymentOutcome::Deleted => deployment.confirm_deletion(now),
            DeploymentOutcome::Running => deployment.confirm_running(now),
            DeploymentOutcome::Failed => deployment.confirm_failed(now),
        };

        if changed {
            self.deployment_repository.update(deployment).await?;
        }

        Ok(changed)
    }

    async fn record_heartbeat(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
    ) -> Result<bool, CoreError> {
        // Same rule as claim_actions and actions:ack -- only Herald reports for
        // a data plane, and a caller able to forge a heartbeat could keep a dead
        // cluster receiving deployments.
        let client_id = identity.username();
        if !client_id.contains("herald-service") {
            return Err(CoreError::PermissionDenied {
                reason: "only herald can report a data plane heartbeat".to_string(),
            });
        }

        self.dataplane_repository
            .touch_last_seen(&dataplane_id, Utc::now())
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataplane::value_objects::{Capacity, DataPlaneAllocation};
    use crate::{
        dataplane::ports::MockDataPlaneRepository,
        deployments::{
            DeploymentKind, DeploymentName, DeploymentStatus, ports::MockDeploymentRepository,
        },
        organisation::OrganisationId,
        user::UserId,
    };
    use chrono::{TimeZone, Utc};

    fn deployment_with_id(id: Uuid, created_at: chrono::DateTime<Utc>) -> Deployment {
        Deployment {
            id: DeploymentId(id),
            organisation_id: OrganisationId(Uuid::new_v4()),
            dataplane_id: DataPlaneId(Uuid::new_v4()),
            name: DeploymentName("deployment".to_string()),
            resources: crate::dataplane::value_objects::DeploymentResources::DEFAULT,
            kind: DeploymentKind::Ferriskey,
            version: crate::version::Version::new(1, 0, 0),
            status: DeploymentStatus::Successful,
            namespace: "ns".to_string(),
            created_by: UserId(Uuid::new_v4()),
            created_at,
            updated_at: created_at,
            deployed_at: None,
            deleted_at: None,
        }
    }

    fn service_with_deployments(
        deployments: Vec<Deployment>,
    ) -> DataPlaneServiceImpl<MockDataPlaneRepository, MockDeploymentRepository> {
        let dataplane_repository = MockDataPlaneRepository::new();
        let mut deployment_repository = MockDeploymentRepository::new();
        deployment_repository
            .expect_list_by_dataplane()
            .return_once(move |_| {
                let deployments = deployments.clone();
                Box::pin(async move { Ok(deployments) })
            });

        DataPlaneServiceImpl::new(
            dataplane_repository,
            deployment_repository,
            Duration::seconds(90),
        )
    }

    fn operator() -> Identity {
        Identity::User(aether_auth::User {
            id: "operator".to_string(),
            username: "operator".to_string(),
            email: None,
            name: None,
            roles: vec!["aether-operator".to_string()],
        })
    }

    fn customer() -> Identity {
        Identity::User(aether_auth::User {
            id: "customer".to_string(),
            username: "customer".to_string(),
            email: None,
            name: None,
            roles: vec![],
        })
    }

    /// Data planes are infrastructure. A customer knowing which cluster hosts
    /// them, how full it is, or that other clusters exist is a leak of the
    /// installation's shape, not a feature.
    #[tokio::test]
    async fn a_customer_cannot_see_the_data_plane_inventory() {
        let service = DataPlaneServiceImpl::new(
            MockDataPlaneRepository::new(),
            MockDeploymentRepository::new(),
            Duration::seconds(90),
        );

        let result = service.list_dataplanes(customer()).await;

        assert!(matches!(result, Err(CoreError::PermissionDenied { .. })));
    }

    #[tokio::test]
    async fn an_operator_can() {
        let mut dataplane_repository = MockDataPlaneRepository::new();
        dataplane_repository
            .expect_list_all()
            .returning(|| Box::pin(async { Ok(Vec::new()) }));

        let service = DataPlaneServiceImpl::new(
            dataplane_repository,
            MockDeploymentRepository::new(),
            Duration::seconds(90),
        );

        assert!(service.list_dataplanes(operator()).await.is_ok());
    }

    /// Choosing where to run is a customer's decision, so the region list is
    /// open to everyone -- and says nothing about what is behind a region.
    #[tokio::test]
    async fn anyone_may_ask_which_regions_are_served() {
        let mut dataplane_repository = MockDataPlaneRepository::new();
        dataplane_repository.expect_list_all().returning(|| {
            Box::pin(async {
                Ok(vec![
                    dataplane_in("fr-par", DataPlaneStatus::Active),
                    dataplane_in("fr-par", DataPlaneStatus::Active),
                    dataplane_in("eu-west", DataPlaneStatus::Provisioning),
                    dataplane_in("dead", DataPlaneStatus::Failed),
                    dataplane_in("off", DataPlaneStatus::Disabled),
                ])
            })
        });

        let service = DataPlaneServiceImpl::new(
            dataplane_repository,
            MockDeploymentRepository::new(),
            Duration::seconds(90),
        );

        let regions = service.list_regions(customer()).await.expect("open to all");

        assert_eq!(
            regions.iter().map(Region::as_str).collect::<Vec<_>>(),
            ["eu-west", "fr-par"],
            "deduplicated, sorted, and without regions nothing can serve"
        );
    }

    fn dataplane_in(region: &str, status: DataPlaneStatus) -> DataPlane {
        DataPlane {
            id: DataPlaneId(Uuid::new_v4()),
            allocation: DataPlaneAllocation::Shared,
            region: Region::new(region),
            status,
            capacity: Capacity::new(4_000, 8_192, 100).unwrap(),
            last_seen_at: None,
            created_at: Utc::now(),
        }
    }

    fn identity(client_id: &str) -> Identity {
        Identity::Client(aether_auth::Client {
            id: "id".to_string(),
            client_id: client_id.to_string(),
            roles: vec![],
            scopes: vec![],
        })
    }

    fn outcome_service(
        deployment: Option<Deployment>,
        expect_update: usize,
    ) -> DataPlaneServiceImpl<MockDataPlaneRepository, MockDeploymentRepository> {
        let mut deployment_repository = MockDeploymentRepository::new();
        deployment_repository
            .expect_get_by_id()
            .returning(move |_| {
                let deployment = deployment.clone();
                Box::pin(async move { Ok(deployment) })
            });
        deployment_repository
            .expect_update()
            .times(expect_update)
            .returning(|_| Box::pin(async { Ok(()) }));

        DataPlaneServiceImpl::new(
            MockDataPlaneRepository::new(),
            deployment_repository,
            Duration::seconds(90),
        )
    }

    fn deleting_deployment(dataplane_id: DataPlaneId) -> Deployment {
        let mut deployment = deployment_with_id(Uuid::new_v4(), Utc::now());
        deployment.dataplane_id = dataplane_id;
        deployment.status = crate::deployments::DeploymentStatus::Deleting;
        deployment
    }

    /// The gap this closes: a deleted deployment sat in `deleting` for ever,
    /// because nothing ever told the control plane the resources were gone.
    #[tokio::test]
    async fn a_deletion_reported_by_the_data_plane_is_recorded() {
        let dataplane_id = DataPlaneId(Uuid::new_v4());
        let deployment = deleting_deployment(dataplane_id);
        let command = ReportDeploymentOutcomeCommand {
            dataplane_id,
            deployment_id: deployment.id,
            outcome: DeploymentOutcome::Deleted,
        };
        let service = outcome_service(Some(deployment), 1);

        let recorded = service
            .report_outcome(identity("service-account-herald-service"), command)
            .await
            .expect("herald may report");

        assert!(recorded);
    }

    /// Same rule as claim, ack and heartbeat. A caller able to forge an outcome
    /// could mark a live deployment deleted.
    #[tokio::test]
    async fn only_herald_may_report_an_outcome() {
        let dataplane_id = DataPlaneId(Uuid::new_v4());
        let deployment = deleting_deployment(dataplane_id);
        let command = ReportDeploymentOutcomeCommand {
            dataplane_id,
            deployment_id: deployment.id,
            outcome: DeploymentOutcome::Deleted,
        };
        let service = outcome_service(Some(deployment), 0);

        let result = service.report_outcome(identity("console"), command).await;

        assert!(matches!(result, Err(CoreError::PermissionDenied { .. })));
    }

    /// A data plane may only speak for what runs on it. Without this a
    /// misconfigured or compromised Herald could delete another data plane's
    /// deployments.
    #[tokio::test]
    async fn a_data_plane_cannot_report_about_another_data_planes_deployment() {
        let deployment = deleting_deployment(DataPlaneId(Uuid::new_v4()));
        let command = ReportDeploymentOutcomeCommand {
            dataplane_id: DataPlaneId(Uuid::new_v4()),
            deployment_id: deployment.id,
            outcome: DeploymentOutcome::Deleted,
        };
        let service = outcome_service(Some(deployment), 0);

        let result = service
            .report_outcome(identity("service-account-herald-service"), command)
            .await;

        assert!(matches!(result, Err(CoreError::PermissionDenied { .. })));
    }

    /// Reports are at-least-once and can outlive the row they describe.
    /// Answering with an error would have the data plane retry for ever.
    #[tokio::test]
    async fn a_report_for_an_unknown_deployment_is_not_an_error() {
        let command = ReportDeploymentOutcomeCommand {
            dataplane_id: DataPlaneId(Uuid::new_v4()),
            deployment_id: crate::deployments::DeploymentId(Uuid::new_v4()),
            outcome: DeploymentOutcome::Deleted,
        };
        let service = outcome_service(None, 0);

        let recorded = service
            .report_outcome(identity("service-account-herald-service"), command)
            .await
            .expect("an unknown deployment is not a failure");

        assert!(!recorded);
    }

    #[tokio::test]
    async fn get_deployments_in_dataplane_applies_shard_partitioning() {
        let dataplane_id = DataPlaneId(Uuid::new_v4());
        let deployments: Vec<Deployment> = (0..8)
            .map(|i| {
                deployment_with_id(
                    Uuid::from_u128((i + 1) as u128),
                    Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, i as u32).unwrap(),
                )
            })
            .collect();
        let service = service_with_deployments(deployments);

        let shard_one = service
            .get_deployments_in_dataplane(
                operator(),
                dataplane_id,
                ListDataPlaneDeploymentsCommand::new(Some(1), Some(4), Some(50), None).unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(shard_one.len(), 2);
        assert!(
            shard_one
                .iter()
                .all(|deployment| (deployment.id.0.as_u128() % 4) == 1)
        );
    }

    #[tokio::test]
    async fn get_deployments_in_dataplane_applies_cursor_and_limit() {
        let dataplane_id = DataPlaneId(Uuid::new_v4());
        let mut deployments: Vec<Deployment> = (0..4)
            .map(|i| {
                deployment_with_id(
                    Uuid::from_u128((100 + i) as u128),
                    Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, (10 - i) as u32)
                        .unwrap(),
                )
            })
            .collect();
        deployments.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| b.id.0.cmp(&a.id.0))
        });
        let cursor = deployments[1].id.to_string();

        let service = service_with_deployments(deployments.clone());
        let page = service
            .get_deployments_in_dataplane(
                operator(),
                dataplane_id,
                ListDataPlaneDeploymentsCommand::new(Some(0), Some(1), Some(2), Some(cursor))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(page.len(), 2);
        assert_eq!(page[0].id, deployments[2].id);
        assert_eq!(page[1].id, deployments[3].id);
    }
}
