use aether_auth::Identity;
use chrono::{Duration, Utc};

use crate::dataplane::herald_identity::{RegisteredDataPlane, speaking_for};
use crate::dataplane::ports::HeraldIdentityProvisioner;
use crate::platform::{PlatformRight, ports::PlatformPolicy};
use crate::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        ports::{DataPlaneRepository, DataPlaneService},
        value_objects::{
            CreateDataplaneCommand, DataPlaneId, DataPlaneStatus, ListDataPlaneDeploymentsCommand,
            Region, ServiceIntent,
        },
    },
    deployments::{
        Deployment, DeploymentId,
        commands::{DeploymentOutcome, ReportDeploymentOutcomeCommand},
        ports::DeploymentRepository,
    },
    version::Version,
};
use uuid::Uuid;

#[derive(Debug)]
pub struct DataPlaneServiceImpl<DP, D, P, I>
where
    DP: DataPlaneRepository,
    D: DeploymentRepository,
    P: PlatformPolicy,
    I: HeraldIdentityProvisioner,
{
    dataplane_repository: DP,
    deployment_repository: D,
    heartbeat_window: Duration,

    /// `None` when this installation has no realm administrator configured,
    /// and therefore cannot give a cluster an identity of its own.
    identities: Option<I>,

    /// Carried rather than checked by the caller, so there is no way to build
    /// this service and reach its methods without one. The same reason
    /// [`crate::deployments::service::DeploymentServiceImpl`] carries its own.
    policy: P,
}

impl<DP, D, P, I> DataPlaneServiceImpl<DP, D, P, I>
where
    DP: DataPlaneRepository,
    D: DeploymentRepository,
    P: PlatformPolicy,
    I: HeraldIdentityProvisioner,
{
    pub fn new(
        dataplane_repository: DP,
        deployment_repository: D,
        heartbeat_window: Duration,
        policy: P,
        identities: Option<I>,
    ) -> Self {
        Self {
            dataplane_repository,
            deployment_repository,
            heartbeat_window,
            policy,
            identities,
        }
    }

    /// Gives a data plane an identity of its own and records what it is.
    ///
    /// The binding is saved before the secret is handed out. A secret whose
    /// binding failed to save is one the cluster would authenticate with and
    /// the control plane would not recognise -- which reads, from the
    /// operator's side, as a credential that simply does not work.
    async fn mint_identity(&self, dataplane: &mut DataPlane) -> Result<Option<String>, CoreError> {
        let Some(identities) = self.identities.as_ref() else {
            return Ok(None);
        };

        let minted = identities.mint(dataplane.id).await?;
        dataplane.herald = Some(minted.binding);
        self.dataplane_repository.save(dataplane).await?;

        Ok(Some(minted.secret))
    }

    pub fn heartbeat_window(&self) -> Duration {
        self.heartbeat_window
    }
}

impl<DP, D, P, I> DataPlaneService for DataPlaneServiceImpl<DP, D, P, I>
where
    DP: DataPlaneRepository,
    D: DeploymentRepository,
    P: PlatformPolicy,
    I: HeraldIdentityProvisioner,
{
    async fn create_dataplane(
        &self,
        identity: Identity,
        command: CreateDataplaneCommand,
    ) -> Result<RegisteredDataPlane, CoreError> {
        // Changing the fleet, not reading it. Somebody who may see which
        // clusters exist is not thereby somebody who may add one.
        self.policy
            .require(identity, PlatformRight::OperateFleet)
            .await?;

        let mut dataplane = DataPlane::new(command.allocation, command.region, command.capacity);
        self.dataplane_repository.save(&dataplane).await?;

        let herald_secret = self.mint_identity(&mut dataplane).await?;

        Ok(RegisteredDataPlane {
            dataplane,
            herald_secret,
        })
    }

    async fn set_dataplane_service(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
        service: ServiceIntent,
    ) -> Result<DataPlane, CoreError> {
        // Changing what the fleet will accept, not reading it. The same right
        // that registers a cluster is the one that stops work going to it.
        self.policy
            .require(identity, PlatformRight::OperateFleet)
            .await?;

        let mut dataplane = self
            .dataplane_repository
            .find_by_id(&dataplane_id)
            .await?
            .ok_or(CoreError::DataPlaneNotFound { id: dataplane_id })?;

        match service {
            ServiceIntent::Draining => dataplane.drain(),
            ServiceIntent::Disabled => dataplane.disable(),
            ServiceIntent::InService => dataplane.return_to_service()?,
        }

        self.dataplane_repository.save(&dataplane).await?;

        Ok(dataplane)
    }

    async fn reissue_herald_credential(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
    ) -> Result<RegisteredDataPlane, CoreError> {
        self.policy
            .require(identity, PlatformRight::OperateFleet)
            .await?;

        let mut dataplane = self
            .dataplane_repository
            .find_by_id(&dataplane_id)
            .await?
            .ok_or(CoreError::DataPlaneNotFound { id: dataplane_id })?;

        let herald_secret = self.mint_identity(&mut dataplane).await?;

        Ok(RegisteredDataPlane {
            dataplane,
            herald_secret,
        })
    }

    async fn list_dataplanes(&self, identity: Identity) -> Result<Vec<DataPlane>, CoreError> {
        self.policy
            .require(identity, PlatformRight::ViewEstate)
            .await?;

        self.dataplane_repository.list_all().await
    }

    async fn get_dataplane(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
    ) -> Result<DataPlane, CoreError> {
        self.policy
            .require(identity, PlatformRight::ViewEstate)
            .await?;

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
        //
        // The Herald path first, and it must be *this* data plane's: a cluster
        // reading another's deployments learns what somebody else runs, which
        // is the same leak the platform listing exists to gate.
        match speaking_for(&self.dataplane_repository, &identity).await {
            Ok(speaking) => speaking.is(dataplane_id)?,
            Err(_) => {
                self.policy
                    .require(identity.clone(), PlatformRight::ViewEstate)
                    .await?
            }
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
        let speaking = speaking_for(&self.dataplane_repository, &identity).await?;

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

        // Against the credential, not against the request. Both sides of this
        // used to come from the caller, so it caught a misconfigured Herald
        // and nothing else.
        speaking.is(deployment.dataplane_id)?;

        let now = Utc::now();
        let changed = match command.outcome {
            DeploymentOutcome::Deleted => deployment.confirm_deletion(now),
            DeploymentOutcome::Running => {
                deployment.confirm_running(now, command.observed_version.clone())
            }
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
        operator_version: Option<Version>,
        gateway_address: Option<String>,
    ) -> Result<bool, CoreError> {
        let speaking = speaking_for(&self.dataplane_repository, &identity).await?;

        // A heartbeat for somebody else's cluster would keep a dead one
        // receiving deployments.
        speaking.is(dataplane_id)?;

        self.dataplane_repository
            .touch_last_seen(&dataplane_id, Utc::now(), operator_version, gateway_address)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataplane::herald_identity::NoIdentities;
    use crate::dataplane::value_objects::{Capacity, DataPlaneAllocation};
    use crate::platform::fixtures::Granting;
    use crate::{
        dataplane::ports::MockDataPlaneRepository,
        deployments::{
            DeploymentKind, DeploymentName, DeploymentStatus, ports::MockDeploymentRepository,
        },
        organisation::OrganisationId,
        user::UserId,
    };
    use chrono::{TimeZone, Utc};
    use std::sync::{Arc, Mutex};

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
            environment: crate::deployments::environment::Environment::Development,
            offer: None,
            restored_from: None,
            created_by: UserId(Uuid::new_v4()),
            created_at,
            updated_at: created_at,
            deployed_at: None,
            deleted_at: None,
            auto_upgrade: Default::default(),
            maintenance_window: None,
            network_access: crate::deployments::network::NetworkAccess::Open,
        }
    }

    /// The whole point of splitting the right. Before this, anybody who could
    /// look at the fleet could also add to it -- and after #238, take a backup
    /// of any tenant.
    #[tokio::test]
    async fn seeing_the_fleet_is_not_permission_to_change_it() {
        let service = DataPlaneServiceImpl::new(
            MockDataPlaneRepository::new(),
            MockDeploymentRepository::new(),
            Duration::seconds(90),
            Granting::only(crate::platform::PlatformRight::ViewEstate),
            None::<NoIdentities>,
        );

        let refused = service
            .create_dataplane(
                identity("somebody"),
                CreateDataplaneCommand {
                    allocation: DataPlaneAllocation::Shared,
                    region: Region::new("fr-par"),
                    capacity: Capacity::new(1000, 1024, 10).unwrap(),
                },
            )
            .await
            .expect_err("a reader registered a data plane");

        let CoreError::MissingPlatformRight { right } = refused else {
            panic!("the refusal did not name a right: {refused}");
        };
        assert_eq!(right, "operate_fleet");
    }

    /// The other side of the same rule: the right that is granted is the right
    /// that works. Without this, the test above would pass on a service that
    /// refuses everybody.
    #[tokio::test]
    async fn the_fleet_right_registers_a_data_plane() {
        let mut dataplanes = MockDataPlaneRepository::new();
        dataplanes
            .expect_save()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));

        let registered = DataPlaneServiceImpl::new(
            dataplanes,
            MockDeploymentRepository::new(),
            Duration::seconds(90),
            Granting::only(crate::platform::PlatformRight::OperateFleet),
            None::<NoIdentities>,
        )
        .create_dataplane(
            identity("somebody"),
            CreateDataplaneCommand {
                allocation: DataPlaneAllocation::Shared,
                region: Region::new("fr-par"),
                capacity: Capacity::new(1000, 1024, 10).unwrap(),
            },
        )
        .await;

        assert!(registered.is_ok(), "{registered:?}");
    }

    fn plane(status: DataPlaneStatus, last_seen_at: Option<chrono::DateTime<Utc>>) -> DataPlane {
        DataPlane {
            id: DataPlaneId(Uuid::new_v4()),
            allocation: DataPlaneAllocation::Shared,
            region: Region::new("fr-par"),
            status,
            capacity: Capacity::new(1000, 1024, 10).unwrap(),
            last_seen_at,
            created_at: Utc::now(),
            herald: None,
            operator_version: None,
            gateway_address: None,
        }
    }

    /// Saved through the repository, so the test sees what was written rather
    /// than what the service happened to return.
    fn holding(existing: DataPlane) -> (MockDataPlaneRepository, Arc<Mutex<Vec<DataPlane>>>) {
        let written: Arc<Mutex<Vec<DataPlane>>> = Arc::new(Mutex::new(Vec::new()));

        let mut dataplanes = MockDataPlaneRepository::new();
        let found = existing.clone();
        dataplanes.expect_find_by_id().returning(move |_| {
            let found = found.clone();
            Box::pin(async move { Ok(Some(found)) })
        });

        let kept = Arc::clone(&written);
        dataplanes.expect_save().returning(move |dataplane| {
            kept.lock().expect("the writes").push(dataplane.clone());
            Box::pin(async { Ok(()) })
        });

        (dataplanes, written)
    }

    fn fleet(
        dataplanes: MockDataPlaneRepository,
        granting: Granting,
    ) -> DataPlaneServiceImpl<
        MockDataPlaneRepository,
        MockDeploymentRepository,
        Granting,
        NoIdentities,
    > {
        DataPlaneServiceImpl::new(
            dataplanes,
            MockDeploymentRepository::new(),
            Duration::seconds(90),
            granting,
            None::<NoIdentities>,
        )
    }

    #[tokio::test]
    async fn an_operator_drains_a_data_plane() {
        let (dataplanes, written) = holding(plane(DataPlaneStatus::Active, Some(Utc::now())));

        let drained = fleet(
            dataplanes,
            Granting::only(crate::platform::PlatformRight::OperateFleet),
        )
        .set_dataplane_service(
            identity("somebody"),
            DataPlaneId(Uuid::new_v4()),
            ServiceIntent::Draining,
        )
        .await
        .expect("drained");

        assert_eq!(drained.status, DataPlaneStatus::Draining);
        assert_eq!(
            written.lock().expect("the writes")[0].status,
            DataPlaneStatus::Draining,
            "and it was written, not only returned"
        );
    }

    /// A cluster that has answered before comes back active; one that never
    /// has rejoins the path a freshly registered plane follows, where the
    /// heartbeat is what promotes it.
    #[tokio::test]
    async fn returning_to_service_puts_a_plane_back_on_the_path_it_was_on() {
        let (reported, _) = holding(plane(DataPlaneStatus::Draining, Some(Utc::now())));
        let back = fleet(
            reported,
            Granting::only(crate::platform::PlatformRight::OperateFleet),
        )
        .set_dataplane_service(
            identity("somebody"),
            DataPlaneId(Uuid::new_v4()),
            ServiceIntent::InService,
        )
        .await
        .expect("back");

        assert_eq!(back.status, DataPlaneStatus::Active);

        let (never, _) = holding(plane(DataPlaneStatus::Disabled, None));
        let back = fleet(
            never,
            Granting::only(crate::platform::PlatformRight::OperateFleet),
        )
        .set_dataplane_service(
            identity("somebody"),
            DataPlaneId(Uuid::new_v4()),
            ServiceIntent::InService,
        )
        .await
        .expect("back");

        assert_eq!(back.status, DataPlaneStatus::Provisioning);
    }

    /// Seeing the fleet is not permission to change what it accepts.
    #[tokio::test]
    async fn seeing_the_fleet_is_not_permission_to_take_a_plane_out_of_service() {
        let (dataplanes, written) = holding(plane(DataPlaneStatus::Active, Some(Utc::now())));

        let refused = fleet(
            dataplanes,
            Granting::only(crate::platform::PlatformRight::ViewEstate),
        )
        .set_dataplane_service(
            identity("somebody"),
            DataPlaneId(Uuid::new_v4()),
            ServiceIntent::Disabled,
        )
        .await
        .expect_err("a reader drained a cluster");

        let CoreError::MissingPlatformRight { right } = refused else {
            panic!("the refusal did not name a right: {refused}");
        };
        assert_eq!(right, "operate_fleet");
        assert!(written.lock().expect("the writes").is_empty());
    }

    /// Herald holds no platform right at all: it speaks for a cluster rather
    /// than for somebody. If this started needing one, every data plane would
    /// stop claiming its work at once.
    #[tokio::test]
    async fn herald_reads_its_own_deployments_without_a_platform_right() {
        let mut deployments = MockDeploymentRepository::new();
        deployments
            .expect_list_by_dataplane()
            .returning(|_| Box::pin(async { Ok(vec![]) }));

        let asking_about = DataPlaneId(uuid::Uuid::new_v4());
        let mut dataplanes = MockDataPlaneRepository::new();
        speaking_as(&mut dataplanes, asking_about);

        let listed = DataPlaneServiceImpl::new(
            dataplanes,
            deployments,
            Duration::seconds(90),
            Granting::nothing(),
            None::<NoIdentities>,
        )
        .get_deployments_in_dataplane(
            identity("service-account-herald-service"),
            asking_about,
            ListDataPlaneDeploymentsCommand::new(None, None, None, None).unwrap(),
        )
        .await;

        assert!(listed.is_ok(), "{listed:?}");
    }

    /// Answers the resolution every Herald path now makes, with the data
    /// plane the test is about.
    fn speaking_as(dataplanes: &mut MockDataPlaneRepository, id: DataPlaneId) {
        dataplanes
            .expect_find_by_herald_subject()
            .returning(move |_| {
                let mut dataplane = DataPlane::new(
                    DataPlaneAllocation::Shared,
                    Region::new("somewhere"),
                    Capacity::new(1000, 1024, 10).expect("non-zero"),
                );
                dataplane.id = id;

                Box::pin(async move { Ok(Some(dataplane)) })
            });
    }

    fn service_with_deployments(
        deployments: Vec<Deployment>,
    ) -> DataPlaneServiceImpl<
        MockDataPlaneRepository,
        MockDeploymentRepository,
        Granting,
        NoIdentities,
    > {
        // Nobody: these tests read the estate as an operator, and the
        // resolution answering "not a Herald" is what sends them to the
        // platform right.
        let mut dataplane_repository = MockDataPlaneRepository::new();
        dataplane_repository
            .expect_find_by_herald_subject()
            .returning(|_| Box::pin(async { Ok(None) }));
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
            Granting::everything(),
            None::<NoIdentities>,
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
            // What "customer" means now: granted nothing. It used to mean
            // "carries no realm role", which was a property of their token
            // rather than of anything somebody decided.
            Granting::nothing(),
            None::<NoIdentities>,
        );

        let result = service.list_dataplanes(customer()).await;

        assert!(matches!(
            result,
            Err(CoreError::MissingPlatformRight { .. })
        ));
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
            Granting::everything(),
            None::<NoIdentities>,
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
            Granting::everything(),
            None::<NoIdentities>,
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
            herald: None,
            id: DataPlaneId(Uuid::new_v4()),
            allocation: DataPlaneAllocation::Shared,
            region: Region::new(region),
            status,
            capacity: Capacity::new(4_000, 8_192, 100).unwrap(),
            last_seen_at: None,
            created_at: Utc::now(),
            operator_version: None,
            gateway_address: None,
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

    /// The same, speaking as a data plane the deployment is not on.
    fn outcome_service_speaking_elsewhere(
        deployment: Option<Deployment>,
    ) -> DataPlaneServiceImpl<
        MockDataPlaneRepository,
        MockDeploymentRepository,
        Granting,
        NoIdentities,
    > {
        let mut dataplanes = MockDataPlaneRepository::new();
        speaking_as(&mut dataplanes, DataPlaneId(Uuid::new_v4()));

        let mut deployment_repository = MockDeploymentRepository::new();
        deployment_repository
            .expect_get_by_id()
            .returning(move |_| {
                let deployment = deployment.clone();
                Box::pin(async move { Ok(deployment) })
            });
        deployment_repository.expect_update().never();

        DataPlaneServiceImpl::new(
            dataplanes,
            deployment_repository,
            Duration::seconds(90),
            Granting::everything(),
            None::<NoIdentities>,
        )
    }

    fn outcome_service(
        deployment: Option<Deployment>,
        expect_update: usize,
    ) -> DataPlaneServiceImpl<
        MockDataPlaneRepository,
        MockDeploymentRepository,
        Granting,
        NoIdentities,
    > {
        // Speaking as the data plane the deployment is on, which is what a
        // correctly configured Herald is. The test that asserts the refusal
        // builds its own, speaking as somebody else.
        let speaking_as_id = deployment
            .as_ref()
            .map(|deployment| deployment.dataplane_id)
            .unwrap_or(DataPlaneId(Uuid::new_v4()));
        let mut dataplanes = MockDataPlaneRepository::new();
        speaking_as(&mut dataplanes, speaking_as_id);

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
            dataplanes,
            deployment_repository,
            Duration::seconds(90),
            Granting::everything(),
            None::<NoIdentities>,
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
            observed_version: None,
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
            observed_version: None,
            dataplane_id,
            deployment_id: deployment.id,
            outcome: DeploymentOutcome::Deleted,
        };
        let service = outcome_service_speaking_elsewhere(Some(deployment));

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
            observed_version: None,
            dataplane_id: DataPlaneId(Uuid::new_v4()),
            deployment_id: deployment.id,
            outcome: DeploymentOutcome::Deleted,
        };
        let service = outcome_service_speaking_elsewhere(Some(deployment));

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
            observed_version: None,
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
