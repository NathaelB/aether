use crate::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        ports::DataPlaneRepository,
        provisioner::{ClusterProvisioner, ProvisionRequest},
        value_objects::{
            DataPlaneMode, DataPlaneStatus, DeploymentResources, PlacementPolicy, PlacementRequest,
            PlacementWindows, Region,
        },
    },
    deployments::{
        Deployment, DeploymentId,
        commands::{CreateDeploymentCommand, UpdateDeploymentCommand},
        ports::{DeploymentRepository, DeploymentService},
    },
    organisation::OrganisationId,
    user::ports::UserRepository,
};
use chrono::{Duration, Utc};
use tracing::{error, info};

/// Refuses an operation that would land on top of one already rewriting the
/// instance. Stated once so the three call sites cannot drift.
fn refuse_if_busy(deployment: &Deployment, operation: &str) -> Result<(), CoreError> {
    if !deployment.is_busy() {
        return Ok(());
    }

    Err(CoreError::DeploymentBusy {
        deployment: deployment.id.0,
        status: deployment.status.to_string(),
        operation: operation.to_string(),
    })
}

#[derive(Debug)]
pub struct DeploymentServiceImpl<D, U, DP, CP>
where
    D: DeploymentRepository,
    U: UserRepository,
    DP: DataPlaneRepository,
    CP: ClusterProvisioner,
{
    deployment_repository: D,
    user_repository: U,
    dataplane_repository: DP,
    provisioner: CP,
    windows: PlacementWindows,
}

impl<D, U, DP, CP> DeploymentServiceImpl<D, U, DP, CP>
where
    D: DeploymentRepository,
    U: UserRepository,
    DP: DataPlaneRepository,
    CP: ClusterProvisioner,
{
    pub fn new(
        deployment_repository: D,
        user_repository: U,
        dataplane_repository: DP,
        provisioner: CP,
        windows: PlacementWindows,
    ) -> Self {
        Self {
            deployment_repository,
            user_repository,
            dataplane_repository,
            provisioner,
            windows,
        }
    }

    /// Placement for `DataPlaneMode::Shared`, unchanged from before this data
    /// plane learned to provision anything: the same `find_available` query,
    /// and the same two-way distinction between "full" and "unknown region"
    /// once it fails. The provisioner is never consulted on this path -- a
    /// shared deployment either fits on an existing data plane or it does
    /// not; there is no cluster to create on its behalf.
    async fn place_on_shared(
        &self,
        organisation_id: OrganisationId,
        region: &Region,
        mode: DataPlaneMode,
        resources: DeploymentResources,
    ) -> Result<DataPlane, CoreError> {
        let dataplane = self
            .dataplane_repository
            .find_available(PlacementRequest {
                region: Some(region.clone()),
                organisation_id,
                mode,
                resources,
                // Spreading, as it has always done -- now stated rather than
                // buried in an ORDER BY. Switching to packing is one value.
                policy: PlacementPolicy::default(),
                seen_since: Utc::now() - self.windows.heartbeat_window,
            })
            .await?;

        // Placement failed for one of two reasons the caller acts on
        // differently, so the answer is only computed once it has actually
        // failed -- the happy path pays nothing for the distinction.
        match dataplane {
            Some(dataplane) => Ok(dataplane),
            None => {
                let region_name = region.as_str().to_string();

                Err(
                    if self.dataplane_repository.region_is_served(region).await? {
                        error!(region = %region_name, ?mode, "no data plane with room");
                        CoreError::NoDataPlaneAvailable {
                            region: region_name,
                            mode: format!("{mode:?}").to_lowercase(),
                        }
                    } else {
                        error!(region = %region_name, "region is not served");
                        CoreError::UnknownRegion {
                            region: region_name,
                        }
                    },
                )
            }
        }
    }

    /// Placement for `DataPlaneMode::Dedicated`. Never touches
    /// `find_available`: a dedicated data plane is looked up by owner, not
    /// chosen from a pool of candidates.
    ///
    /// Liveness is deliberately not required here, unlike shared placement's
    /// `accepts_placement`. A freshly provisioned cluster has never sent a
    /// heartbeat, so requiring `Reachable` would mean the first deployment on
    /// an organisation's own cluster could never be created. And unlike
    /// shared placement there is nowhere else to put it -- it is that
    /// organisation's cluster or nothing. The deployment is left in `Pending`
    /// until that data plane's Herald comes up and claims the action, which
    /// is the pull model working as designed, not a failure to handle.
    async fn place_on_dedicated(
        &self,
        organisation_id: OrganisationId,
        region: &Region,
        resources: DeploymentResources,
    ) -> Result<DataPlane, CoreError> {
        // Looked up first, and unconditionally: without this, two dedicated
        // deployments created before the first heartbeat would each
        // provision a cluster for the same organisation.
        let existing = self
            .dataplane_repository
            .find_dedicated_for_organisation(&organisation_id, region)
            .await?;

        if let Some(dataplane) = existing {
            // Liveness is skipped on this path, but status is not. Those are
            // different questions: liveness asks whether a cluster has
            // reported yet, and a freshly provisioned one never has. Status is
            // what an operator or a failed provision decided, and a plane that
            // is `Failed`, `Disabled` or `Draining` will not run what is put on
            // it -- accepting a deployment onto one would leave it `Pending`
            // for ever with nothing to explain why.
            //
            // Provisioning still accepts: that is the state every dedicated
            // plane passes through between being created and its Herald
            // reporting in, and refusing it would make the first deployment on
            // an organisation's own cluster impossible.
            // The trust `Provisioning` gets above has an upper bound. Past
            // it, a plane that has still never reported is not coming up, and
            // placing on it chooses an outcome nobody wants over an error.
            let stuck =
                dataplane.is_stuck_provisioning(Utc::now(), self.windows.provisioning_timeout);

            if stuck
                || !matches!(
                    dataplane.status,
                    DataPlaneStatus::Active | DataPlaneStatus::Provisioning
                )
            {
                error!(
                    region = %region.as_str(),
                    status = ?dataplane.status,
                    stuck,
                    "the organisation's dedicated data plane cannot accept placement"
                );

                return Err(CoreError::NoDataPlaneAvailable {
                    region: region.as_str().to_string(),
                    mode: "dedicated".to_string(),
                });
            }

            let placed = self
                .deployment_repository
                .list_by_dataplane(&dataplane.id)
                .await?;
            // Filtered here rather than in the query: `list_by_dataplane` also
            // serves Herald's enumeration of its own data plane, and narrowing
            // a Herald-facing endpoint to fix a placement sum is a change with
            // a much larger blast radius than the bug. What placement must not
            // do is reserve room for a deployment that no longer exists.
            let used = placed
                .iter()
                .filter(|deployment| deployment.deleted_at.is_none())
                .fold(
                    DeploymentResources {
                        cpu_millis: 0,
                        memory_mib: 0,
                        storage_gib: 0,
                    },
                    |acc, deployment| DeploymentResources {
                        cpu_millis: acc
                            .cpu_millis
                            .saturating_add(deployment.resources.cpu_millis),
                        memory_mib: acc
                            .memory_mib
                            .saturating_add(deployment.resources.memory_mib),
                        storage_gib: acc
                            .storage_gib
                            .saturating_add(deployment.resources.storage_gib),
                    },
                );

            return if dataplane.capacity.fits(used, resources) {
                Ok(dataplane)
            } else {
                error!(region = %region.as_str(), "dedicated data plane has no room");
                Err(CoreError::NoDataPlaneAvailable {
                    region: region.as_str().to_string(),
                    mode: "dedicated".to_string(),
                })
            };
        }

        let dataplane = self
            .provisioner
            .provision(ProvisionRequest {
                organisation_id,
                region: region.clone(),
                minimum: resources,
            })
            .await?;

        self.dataplane_repository.save(&dataplane).await?;

        Ok(dataplane)
    }
}

impl<D, U, DP, CP> DeploymentService for DeploymentServiceImpl<D, U, DP, CP>
where
    D: DeploymentRepository,
    U: UserRepository,
    DP: DataPlaneRepository,
    CP: ClusterProvisioner,
{
    async fn create_deployment(
        &self,
        command: CreateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        let user = self
            .user_repository
            .find_by_sub(&command.created_by.to_string())
            .await?
            .ok_or(CoreError::InvalidIdentity)?;

        info!("user {} try to create depliyment", user.email);

        let dataplane = match command.mode {
            DataPlaneMode::Shared => {
                self.place_on_shared(
                    command.organisation_id,
                    &command.region,
                    command.mode,
                    command.resources,
                )
                .await?
            }
            DataPlaneMode::Dedicated => {
                self.place_on_dedicated(command.organisation_id, &command.region, command.resources)
                    .await?
            }
        };

        let now = chrono::Utc::now();
        let deployment = Deployment {
            id: DeploymentId(uuid::Uuid::new_v4()),
            organisation_id: command.organisation_id,
            dataplane_id: dataplane.id,
            name: command.name,
            kind: command.kind,
            version: command.version,
            status: command.status,
            namespace: command.namespace,
            resources: command.resources,
            created_by: user.id,
            created_at: now,
            updated_at: now,
            deployed_at: None,
            deleted_at: None,
            auto_upgrade: Default::default(),
            maintenance_window: None,
        };

        info!(
            "try create new deployment {:?} kind: {}",
            deployment.name, deployment.kind
        );

        self.deployment_repository
            .insert(deployment.clone())
            .await
            .map_err(|e| {
                error!("failed to create deployment: {}", e);
                e
            })?;
        Ok(deployment)
    }

    async fn get_deployment(
        &self,
        deployment_id: DeploymentId,
    ) -> Result<Option<Deployment>, CoreError> {
        self.deployment_repository.get_by_id(deployment_id).await
    }

    async fn get_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<Deployment, CoreError> {
        let deployment = self
            .deployment_repository
            .get_by_id(deployment_id)
            .await?
            .ok_or(CoreError::DeploymentNotFound {
                id: deployment_id.0,
            })?;

        // A deployment belonging to another organisation answers exactly like
        // one that does not exist -- distinguishing the two would confirm its
        // existence to someone who has no business asking.
        if deployment.organisation_id != organisation_id {
            return Err(CoreError::DeploymentNotFound {
                id: deployment_id.0,
            });
        }

        Ok(deployment)
    }

    async fn list_deployments_by_organisation(
        &self,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Deployment>, CoreError> {
        self.deployment_repository
            .list_by_organisation(organisation_id)
            .await
    }

    async fn update_deployment(
        &self,
        deployment_id: DeploymentId,
        command: UpdateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        if command.is_empty() {
            return Err(CoreError::InternalError(
                "Update command cannot be empty".to_string(),
            ));
        }

        let mut deployment = self
            .deployment_repository
            .get_by_id(deployment_id)
            .await?
            .ok_or(CoreError::DeploymentNotFound {
                id: deployment_id.0,
            })?;

        refuse_if_busy(&deployment, "changed")?;

        if let Some(name) = command.name {
            deployment.name = name;
        }
        if let Some(kind) = command.kind {
            deployment.kind = kind;
        }
        if let Some(version) = command.version {
            deployment.version = version;
        }
        if let Some(status) = command.status {
            deployment.status = status;
        }
        if let Some(namespace) = command.namespace {
            deployment.namespace = namespace;
        }
        if let Some(deployed_at) = command.deployed_at {
            deployment.deployed_at = deployed_at;
        }
        if let Some(deleted_at) = command.deleted_at {
            deployment.deleted_at = deleted_at;
        }

        deployment.updated_at = chrono::Utc::now();

        self.deployment_repository
            .update(deployment.clone())
            .await?;
        Ok(deployment)
    }

    async fn update_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
        command: UpdateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        let deployment = self
            .get_deployment_for_organisation(organisation_id, deployment_id)
            .await?;

        self.update_deployment(deployment.id, command).await
    }

    async fn purge_deleted_deployments(&self, retention: Duration) -> Result<u64, CoreError> {
        self.deployment_repository
            .purge_deleted(Utc::now() - retention)
            .await
    }

    async fn delete_deployment(
        &self,
        deployment_id: DeploymentId,
    ) -> Result<Deployment, CoreError> {
        // Read before deleting, not after: the row is soft-deleted, so reading
        // it back would work -- but it would come back already marked, and the
        // action payload would describe a deployment in the state that follows
        // the deletion rather than the one being deleted.
        let deployment = self
            .deployment_repository
            .get_by_id(deployment_id)
            .await?
            .ok_or(CoreError::DeploymentNotFound {
                id: deployment_id.0,
            })?;

        refuse_if_busy(&deployment, "deleted")?;

        self.deployment_repository.delete(deployment_id).await?;

        Ok(deployment)
    }

    async fn delete_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<Deployment, CoreError> {
        let deployment = self
            .get_deployment_for_organisation(organisation_id, deployment_id)
            .await?;

        refuse_if_busy(&deployment, "deleted")?;

        self.deployment_repository.delete(deployment.id).await?;

        Ok(deployment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataplane::value_objects::DeploymentResources;
    use crate::{
        dataplane::{
            entities::DataPlane,
            ports::MockDataPlaneRepository,
            provisioner::MockClusterProvisioner,
            value_objects::{
                Capacity, DataPlaneAllocation, DataPlaneId, DataPlaneMode, DataPlaneStatus, Region,
            },
        },
        deployments::ports::MockDeploymentRepository,
        deployments::{DeploymentKind, DeploymentName, DeploymentStatus},
        user::UserId,
    };
    use chrono::Utc;
    use uuid::Uuid;

    /// Shorthand for the tests below that never expect the provisioner to be
    /// touched at all -- everything on the shared path, and error paths that
    /// return before placement decides anything.
    fn no_provisioning() -> MockClusterProvisioner {
        MockClusterProvisioner::new()
    }

    struct StubUserRepository;

    impl crate::user::ports::UserRepository for StubUserRepository {
        fn upsert_by_email(
            &self,
            user: &crate::user::User,
        ) -> impl std::future::Future<Output = Result<crate::user::User, CoreError>> + Send
        {
            let cloned = crate::user::User {
                id: user.id,
                email: user.email.clone(),
                name: user.name.clone(),
                sub: user.sub.clone(),
                created_at: user.created_at,
                updated_at: user.updated_at,
            };
            async move { Ok(cloned) }
        }

        fn find_by_sub(
            &self,
            sub: &str,
        ) -> impl std::future::Future<Output = Result<Option<crate::user::User>, CoreError>> + Send
        {
            let sub = sub.to_string();
            async move {
                let Ok(parsed) = Uuid::parse_str(&sub) else {
                    return Ok(None);
                };
                Ok(Some(crate::user::User {
                    id: UserId(parsed),
                    email: "user@example.com".to_string(),
                    name: "User".to_string(),
                    sub,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                }))
            }
        }
    }

    fn sample_deployment(
        deployment_id: DeploymentId,
        organisation_id: OrganisationId,
    ) -> Deployment {
        Deployment {
            id: deployment_id,
            organisation_id,
            dataplane_id: DataPlaneId(Uuid::new_v4()),
            name: DeploymentName("app".to_string()),
            kind: DeploymentKind::Keycloak,
            version: crate::version::Version::new(1, 0, 0),
            status: DeploymentStatus::Pending,
            namespace: "default".to_string(),
            resources: DeploymentResources::DEFAULT,
            created_by: UserId(Uuid::new_v4()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deployed_at: None,
            deleted_at: None,
            auto_upgrade: Default::default(),
            maintenance_window: None,
        }
    }

    fn sample_dataplane() -> DataPlane {
        DataPlane {
            id: DataPlaneId(Uuid::new_v4()),
            allocation: DataPlaneAllocation::Shared,
            region: Region::new("local"),
            status: DataPlaneStatus::Active,
            capacity: Capacity::new(5000, 10240, 10).unwrap(),
            last_seen_at: Some(Utc::now()),
            created_at: Utc::now(),
        }
    }

    fn windows() -> PlacementWindows {
        PlacementWindows::new(Duration::seconds(90), Duration::minutes(30))
    }

    /// A dedicated data plane that has never reported -- the state a freshly
    /// provisioned cluster is in, and the one `place_on_dedicated` must still
    /// place on.
    fn dedicated_dataplane(organisation_id: OrganisationId, capacity: Capacity) -> DataPlane {
        DataPlane {
            id: DataPlaneId(Uuid::new_v4()),
            allocation: DataPlaneAllocation::Dedicated { organisation_id },
            region: Region::new("eu-west"),
            status: DataPlaneStatus::Provisioning,
            capacity,
            last_seen_at: None,
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn create_deployment_persists() {
        let mut mock_repo = MockDeploymentRepository::new();
        let mut mock_dataplane_repo = MockDataPlaneRepository::new();
        mock_repo
            .expect_insert()
            .times(1)
            .withf(|deployment| deployment.name.0 == "app")
            .returning(|_| Box::pin(async { Ok(()) }));
        mock_dataplane_repo
            .expect_find_available()
            .times(1)
            .returning(|_| {
                let dataplane = sample_dataplane();
                Box::pin(async move { Ok(Some(dataplane)) })
            });

        let service = DeploymentServiceImpl::new(
            mock_repo,
            StubUserRepository,
            mock_dataplane_repo,
            no_provisioning(),
            windows(),
        );
        let command = CreateDeploymentCommand::new(
            OrganisationId(Uuid::new_v4()),
            DeploymentName("app".to_string()),
            DeploymentKind::Keycloak,
            crate::version::Version::new(1, 0, 0),
            DeploymentStatus::Pending,
            "default".to_string(),
            UserId(Uuid::new_v4()),
            Region::new("fr-par"),
            DataPlaneMode::Shared,
            DeploymentResources::DEFAULT,
        );

        let result = service.create_deployment(command).await;
        assert!(result.is_ok());
    }

    /// #136: a mistyped id and someone else's deployment must be
    /// indistinguishable to the caller. Both are `DeploymentNotFound`, not the
    /// opaque `InternalError` that used to turn either one into a 500.
    #[tokio::test]
    async fn a_deployment_that_does_not_exist_is_not_found_rather_than_broken() {
        let mut mock_repo = MockDeploymentRepository::new();
        let mock_dataplane_repo = MockDataPlaneRepository::new();
        let deployment_id = DeploymentId(Uuid::new_v4());
        let organisation_id = OrganisationId(Uuid::new_v4());

        mock_repo
            .expect_get_by_id()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));

        let service = DeploymentServiceImpl::new(
            mock_repo,
            StubUserRepository,
            mock_dataplane_repo,
            no_provisioning(),
            windows(),
        );
        let result = service
            .get_deployment_for_organisation(organisation_id, deployment_id)
            .await;

        match result {
            Err(CoreError::DeploymentNotFound { id }) => assert_eq!(id, deployment_id.0),
            other => panic!("expected DeploymentNotFound, got {other:?}"),
        }
    }

    /// The other branch of the same guarantee: a deployment that exists, but
    /// under a different organisation, must answer exactly like one that does
    /// not exist at all -- nothing here may hint that it is out there
    /// somewhere else.
    #[tokio::test]
    async fn a_deployment_belonging_to_another_organisation_is_not_found_rather_than_broken() {
        let mut mock_repo = MockDeploymentRepository::new();
        let mock_dataplane_repo = MockDataPlaneRepository::new();
        let deployment_id = DeploymentId(Uuid::new_v4());
        let organisation_id = OrganisationId(Uuid::new_v4());
        let other_org = OrganisationId(Uuid::new_v4());

        let deployment = sample_deployment(deployment_id, other_org);
        mock_repo.expect_get_by_id().times(1).returning(move |_| {
            let deployment = deployment.clone();
            Box::pin(async move { Ok(Some(deployment)) })
        });

        let service = DeploymentServiceImpl::new(
            mock_repo,
            StubUserRepository,
            mock_dataplane_repo,
            no_provisioning(),
            windows(),
        );
        let result = service
            .get_deployment_for_organisation(organisation_id, deployment_id)
            .await;

        match result {
            Err(CoreError::DeploymentNotFound { id }) => assert_eq!(id, deployment_id.0),
            other => panic!("expected DeploymentNotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn update_deployment_rejects_empty_command() {
        let service = DeploymentServiceImpl::new(
            MockDeploymentRepository::new(),
            StubUserRepository,
            MockDataPlaneRepository::new(),
            no_provisioning(),
            windows(),
        );
        let result = service
            .update_deployment(DeploymentId(Uuid::new_v4()), UpdateDeploymentCommand::new())
            .await;

        assert!(matches!(result, Err(CoreError::InternalError(_))));
    }

    #[tokio::test]
    async fn update_deployment_applies_changes() {
        let mut mock_repo = MockDeploymentRepository::new();
        let mock_dataplane_repo = MockDataPlaneRepository::new();
        let deployment_id = DeploymentId(Uuid::new_v4());
        let organisation_id = OrganisationId(Uuid::new_v4());
        let deployment = sample_deployment(deployment_id, organisation_id);

        mock_repo.expect_get_by_id().times(1).returning(move |_| {
            let deployment = deployment.clone();
            Box::pin(async move { Ok(Some(deployment)) })
        });

        mock_repo
            .expect_update()
            .times(1)
            .withf(|deployment| deployment.status == DeploymentStatus::Successful)
            .returning(|_| Box::pin(async { Ok(()) }));

        let service = DeploymentServiceImpl::new(
            mock_repo,
            StubUserRepository,
            mock_dataplane_repo,
            no_provisioning(),
            windows(),
        );
        let command = UpdateDeploymentCommand::new().with_status(DeploymentStatus::Successful);

        let result = service.update_deployment(deployment_id, command).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, DeploymentStatus::Successful);
    }

    #[tokio::test]
    async fn list_deployments_delegates() {
        let mut mock_repo = MockDeploymentRepository::new();
        let mock_dataplane_repo = MockDataPlaneRepository::new();
        let organisation_id = OrganisationId(Uuid::new_v4());
        let deployments = vec![sample_deployment(
            DeploymentId(Uuid::new_v4()),
            organisation_id,
        )];

        mock_repo
            .expect_list_by_organisation()
            .times(1)
            .returning(move |_| {
                let deployments = deployments.clone();
                Box::pin(async move { Ok(deployments) })
            });

        let service = DeploymentServiceImpl::new(
            mock_repo,
            StubUserRepository,
            mock_dataplane_repo,
            no_provisioning(),
            windows(),
        );
        let result = service
            .list_deployments_by_organisation(organisation_id)
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    fn command_for(region: &str, mode: DataPlaneMode) -> CreateDeploymentCommand {
        CreateDeploymentCommand::new(
            OrganisationId(Uuid::new_v4()),
            DeploymentName("app".to_string()),
            DeploymentKind::Keycloak,
            crate::version::Version::new(1, 0, 0),
            DeploymentStatus::Pending,
            "default".to_string(),
            UserId(Uuid::new_v4()),
            Region::new(region),
            mode,
            DeploymentResources::DEFAULT,
        )
    }

    /// The point of the change: what the caller asked for is what reaches the
    /// query. A region silently substituted for another is a deployment in a
    /// jurisdiction nobody chose. Shared is the only mode that still reaches
    /// `find_available` -- dedicated placement is covered separately below,
    /// since it never calls it at all.
    #[tokio::test]
    async fn the_requested_region_and_mode_reach_placement() {
        let mut mock_repo = MockDeploymentRepository::new();
        mock_repo
            .expect_insert()
            .returning(|_| Box::pin(async { Ok(()) }));

        let mut mock_dataplane_repo = MockDataPlaneRepository::new();
        mock_dataplane_repo
            .expect_find_available()
            .times(1)
            .withf(|request| {
                request.region.as_ref().map(|r| r.as_str()) == Some("eu-west")
                    && request.mode == DataPlaneMode::Shared
            })
            .returning(|_| {
                let dataplane = sample_dataplane();
                Box::pin(async move { Ok(Some(dataplane)) })
            });

        let service = DeploymentServiceImpl::new(
            mock_repo,
            StubUserRepository,
            mock_dataplane_repo,
            no_provisioning(),
            windows(),
        );

        let result = service
            .create_deployment(command_for("eu-west", DataPlaneMode::Shared))
            .await;

        assert!(result.is_ok());
    }

    /// A region that is served but full is a "try again later"; the caller can
    /// retry, or wait for capacity to be added.
    #[tokio::test]
    async fn a_served_region_with_no_room_is_reported_as_full() {
        let mut mock_dataplane_repo = MockDataPlaneRepository::new();
        mock_dataplane_repo
            .expect_find_available()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));
        mock_dataplane_repo
            .expect_region_is_served()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));

        let service = DeploymentServiceImpl::new(
            MockDeploymentRepository::new(),
            StubUserRepository,
            mock_dataplane_repo,
            no_provisioning(),
            windows(),
        );

        let result = service
            .create_deployment(command_for("fr-par", DataPlaneMode::Shared))
            .await;

        match result {
            Err(CoreError::NoDataPlaneAvailable { region, mode }) => {
                assert_eq!(region, "fr-par");
                assert_eq!(mode, "shared");
            }
            other => panic!("expected NoDataPlaneAvailable, got {other:?}"),
        }
    }

    /// A region nobody serves is a different answer: retrying will not help,
    /// and the caller asked for something this installation cannot do.
    #[tokio::test]
    async fn an_unserved_region_is_not_reported_as_full() {
        let mut mock_dataplane_repo = MockDataPlaneRepository::new();
        mock_dataplane_repo
            .expect_find_available()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));
        mock_dataplane_repo
            .expect_region_is_served()
            .times(1)
            .withf(|region| region.as_str() == "antarctica")
            .returning(|_| Box::pin(async { Ok(false) }));

        let service = DeploymentServiceImpl::new(
            MockDeploymentRepository::new(),
            StubUserRepository,
            mock_dataplane_repo,
            no_provisioning(),
            windows(),
        );

        let result = service
            .create_deployment(command_for("antarctica", DataPlaneMode::Shared))
            .await;

        match result {
            Err(CoreError::UnknownRegion { region }) => assert_eq!(region, "antarctica"),
            other => panic!("expected UnknownRegion, got {other:?}"),
        }
    }

    /// The distinction costs nothing when placement succeeds: the extra query
    /// is only asked once there is a failure to explain.
    #[tokio::test]
    async fn a_successful_placement_never_asks_whether_the_region_is_served() {
        let mut mock_repo = MockDeploymentRepository::new();
        mock_repo
            .expect_insert()
            .returning(|_| Box::pin(async { Ok(()) }));

        let mut mock_dataplane_repo = MockDataPlaneRepository::new();
        mock_dataplane_repo.expect_find_available().returning(|_| {
            let dataplane = sample_dataplane();
            Box::pin(async move { Ok(Some(dataplane)) })
        });
        mock_dataplane_repo.expect_region_is_served().never();

        let service = DeploymentServiceImpl::new(
            mock_repo,
            StubUserRepository,
            mock_dataplane_repo,
            no_provisioning(),
            windows(),
        );

        assert!(
            service
                .create_deployment(command_for("fr-par", DataPlaneMode::Shared))
                .await
                .is_ok()
        );
    }

    /// The provisioner is the one exception to the pull model, and it must
    /// stay that way: a shared deployment always has a pool to draw from, so
    /// nothing on this path may ever reach for it.
    #[tokio::test]
    async fn a_shared_deployment_never_calls_the_provisioner() {
        let mut mock_repo = MockDeploymentRepository::new();
        mock_repo
            .expect_insert()
            .returning(|_| Box::pin(async { Ok(()) }));

        let mut mock_dataplane_repo = MockDataPlaneRepository::new();
        mock_dataplane_repo.expect_find_available().returning(|_| {
            let dataplane = sample_dataplane();
            Box::pin(async move { Ok(Some(dataplane)) })
        });

        let mut mock_provisioner = MockClusterProvisioner::new();
        mock_provisioner.expect_provision().times(0);
        mock_provisioner.expect_deprovision().times(0);

        let service = DeploymentServiceImpl::new(
            mock_repo,
            StubUserRepository,
            mock_dataplane_repo,
            mock_provisioner,
            windows(),
        );

        let result = service
            .create_deployment(command_for("fr-par", DataPlaneMode::Shared))
            .await;

        assert!(result.is_ok());
    }

    /// The reason `find_dedicated_for_organisation` is asked before
    /// provisioning at all: without it, a dedicated deployment created before
    /// the first heartbeat would provision a cluster every single time.
    #[tokio::test]
    async fn a_dedicated_deployment_with_no_existing_plane_provisions_exactly_once() {
        let organisation_id = OrganisationId(Uuid::new_v4());

        let mut mock_repo = MockDeploymentRepository::new();
        mock_repo
            .expect_insert()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));

        let mut mock_dataplane_repo = MockDataPlaneRepository::new();
        mock_dataplane_repo
            .expect_find_dedicated_for_organisation()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(None) }));
        mock_dataplane_repo
            .expect_save()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));

        let mut mock_provisioner = MockClusterProvisioner::new();
        mock_provisioner
            .expect_provision()
            .times(1)
            .returning(move |request| {
                let dataplane = DataPlane::new(
                    DataPlaneAllocation::Dedicated {
                        organisation_id: request.organisation_id,
                    },
                    request.region,
                    Capacity::new(4_000, 8_192, 100).unwrap(),
                );
                Box::pin(async move { Ok(dataplane) })
            });

        let service = DeploymentServiceImpl::new(
            mock_repo,
            StubUserRepository,
            mock_dataplane_repo,
            mock_provisioner,
            windows(),
        );

        let mut command = command_for("eu-west", DataPlaneMode::Dedicated);
        command.organisation_id = organisation_id;

        let result = service.create_deployment(command).await;

        assert!(result.is_ok());
    }

    /// The double-provisioning bug this ordering exists to prevent: without
    /// looking for an existing dedicated data plane first, a second
    /// deployment for the same organisation would provision a second
    /// cluster.
    #[tokio::test]
    async fn a_second_dedicated_deployment_does_not_provision_a_second_cluster() {
        let organisation_id = OrganisationId(Uuid::new_v4());
        let capacity = Capacity::new(4_000, 8_192, 100).unwrap();

        let mut mock_repo = MockDeploymentRepository::new();
        mock_repo
            .expect_insert()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        mock_repo
            .expect_list_by_dataplane()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));

        let mut mock_dataplane_repo = MockDataPlaneRepository::new();
        mock_dataplane_repo
            .expect_find_dedicated_for_organisation()
            .times(1)
            .returning(move |_, _| {
                let dataplane = dedicated_dataplane(organisation_id, capacity);
                Box::pin(async move { Ok(Some(dataplane)) })
            });
        mock_dataplane_repo.expect_save().times(0);

        let mut mock_provisioner = MockClusterProvisioner::new();
        mock_provisioner.expect_provision().times(0);

        let service = DeploymentServiceImpl::new(
            mock_repo,
            StubUserRepository,
            mock_dataplane_repo,
            mock_provisioner,
            windows(),
        );

        let mut command = command_for("eu-west", DataPlaneMode::Dedicated);
        command.organisation_id = organisation_id;

        let result = service.create_deployment(command).await;

        assert!(result.is_ok());
    }

    /// A dedicated plane whose provisioning failed, that an operator disabled,
    /// or that is draining will not run what is put on it. Accepting the
    /// deployment anyway would leave it `Pending` for ever with nothing on the
    /// deployment itself to explain why -- and provisioning a second cluster
    /// instead would leave the organisation paying for two.
    #[tokio::test]
    async fn a_dedicated_plane_that_cannot_serve_is_refused_rather_than_silently_accepted() {
        for status in [
            DataPlaneStatus::Failed,
            DataPlaneStatus::Disabled,
            DataPlaneStatus::Draining,
        ] {
            let organisation_id = OrganisationId(Uuid::new_v4());
            let capacity = Capacity::new(4_000, 8_192, 100).unwrap();

            let mut mock_repo = MockDeploymentRepository::new();
            mock_repo.expect_insert().times(0);
            // The capacity sum is never reached: a plane that cannot serve is
            // refused before its room is even counted.
            mock_repo.expect_list_by_dataplane().times(0);

            let mut mock_dataplane_repo = MockDataPlaneRepository::new();
            mock_dataplane_repo
                .expect_find_dedicated_for_organisation()
                .times(1)
                .returning(move |_, _| {
                    let mut dataplane = dedicated_dataplane(organisation_id, capacity);
                    dataplane.status = status;
                    Box::pin(async move { Ok(Some(dataplane)) })
                });
            mock_dataplane_repo.expect_save().times(0);

            let mut mock_provisioner = MockClusterProvisioner::new();
            mock_provisioner.expect_provision().times(0);

            let service = DeploymentServiceImpl::new(
                mock_repo,
                StubUserRepository,
                mock_dataplane_repo,
                mock_provisioner,
                windows(),
            );

            let mut command = command_for("eu-west", DataPlaneMode::Dedicated);
            command.organisation_id = organisation_id;

            let result = service.create_deployment(command).await;

            assert!(
                matches!(result, Err(CoreError::NoDataPlaneAvailable { .. })),
                "{status:?} must not accept placement"
            );
        }
    }

    /// #78: the trust `Provisioning` gets has an upper bound.
    ///
    /// A plane that entered `Provisioning` and never came up kept accepting
    /// every dedicated deployment its organisation created, for ever, each one
    /// waiting on a Herald that was never coming. Refusing is not a worse
    /// outcome than that -- it is the only one that says what happened.
    #[tokio::test]
    async fn a_dedicated_plane_that_never_came_up_stops_accepting_deployments() {
        let organisation_id = OrganisationId(Uuid::new_v4());
        let capacity = Capacity::new(4_000, 8_192, 100).unwrap();

        let mut mock_repo = MockDeploymentRepository::new();
        mock_repo.expect_insert().times(0);
        mock_repo.expect_list_by_dataplane().times(0);

        let mut mock_dataplane_repo = MockDataPlaneRepository::new();
        mock_dataplane_repo
            .expect_find_dedicated_for_organisation()
            .times(1)
            .returning(move |_, _| {
                let mut dataplane = dedicated_dataplane(organisation_id, capacity);
                dataplane.created_at = Utc::now() - Duration::hours(4);
                Box::pin(async move { Ok(Some(dataplane)) })
            });
        // Provisioning a second cluster instead would leave the organisation
        // paying for two, neither of which works.
        mock_dataplane_repo.expect_save().times(0);

        let mut mock_provisioner = MockClusterProvisioner::new();
        mock_provisioner.expect_provision().times(0);

        let service = DeploymentServiceImpl::new(
            mock_repo,
            StubUserRepository,
            mock_dataplane_repo,
            mock_provisioner,
            windows(),
        );

        let mut command = command_for("eu-west", DataPlaneMode::Dedicated);
        command.organisation_id = organisation_id;

        let result = service.create_deployment(command).await;

        assert!(
            matches!(result, Err(CoreError::NoDataPlaneAvailable { .. })),
            "a plane stuck provisioning must not absorb the deployment"
        );
    }

    /// The other half of the rule above: `Provisioning` and `Active` do
    /// accept. Provisioning especially -- it is the state every dedicated
    /// plane passes through between being created and its Herald reporting in,
    /// so refusing it would make the first deployment on an organisation's own
    /// cluster impossible.
    #[tokio::test]
    async fn a_dedicated_plane_that_is_provisioning_or_active_still_accepts_placement() {
        for status in [DataPlaneStatus::Provisioning, DataPlaneStatus::Active] {
            let organisation_id = OrganisationId(Uuid::new_v4());
            let capacity = Capacity::new(4_000, 8_192, 100).unwrap();

            let mut mock_repo = MockDeploymentRepository::new();
            mock_repo
                .expect_insert()
                .times(1)
                .returning(|_| Box::pin(async { Ok(()) }));
            mock_repo
                .expect_list_by_dataplane()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));

            let mut mock_dataplane_repo = MockDataPlaneRepository::new();
            mock_dataplane_repo
                .expect_find_dedicated_for_organisation()
                .times(1)
                .returning(move |_, _| {
                    let mut dataplane = dedicated_dataplane(organisation_id, capacity);
                    dataplane.status = status;
                    Box::pin(async move { Ok(Some(dataplane)) })
                });

            let mut mock_provisioner = MockClusterProvisioner::new();
            mock_provisioner.expect_provision().times(0);

            let service = DeploymentServiceImpl::new(
                mock_repo,
                StubUserRepository,
                mock_dataplane_repo,
                mock_provisioner,
                windows(),
            );

            let mut command = command_for("eu-west", DataPlaneMode::Dedicated);
            command.organisation_id = organisation_id;

            assert!(
                service.create_deployment(command).await.is_ok(),
                "{status:?} must accept placement"
            );
        }
    }

    /// Dedicated placement has nowhere else to send an overflowing
    /// deployment -- it is that organisation's cluster or nothing -- so a
    /// full dedicated plane is reported the same way a full shared one is,
    /// rather than triggering a second cluster.
    #[tokio::test]
    async fn a_dedicated_plane_without_room_is_reported_as_full_instead_of_provisioning_again() {
        let organisation_id = OrganisationId(Uuid::new_v4());
        // Exactly the default deployment size -- one already placed leaves no
        // room for another of the same size.
        let capacity = Capacity::new(500, 1_024, 1).unwrap();

        let mut mock_repo = MockDeploymentRepository::new();
        mock_repo
            .expect_list_by_dataplane()
            .times(1)
            .returning(move |dataplane_id| {
                let mut deployment =
                    sample_deployment(DeploymentId(Uuid::new_v4()), organisation_id);
                deployment.dataplane_id = *dataplane_id;
                Box::pin(async move { Ok(vec![deployment]) })
            });

        let mut mock_dataplane_repo = MockDataPlaneRepository::new();
        mock_dataplane_repo
            .expect_find_dedicated_for_organisation()
            .times(1)
            .returning(move |_, _| {
                let dataplane = dedicated_dataplane(organisation_id, capacity);
                Box::pin(async move { Ok(Some(dataplane)) })
            });

        let mut mock_provisioner = MockClusterProvisioner::new();
        mock_provisioner.expect_provision().times(0);

        let service = DeploymentServiceImpl::new(
            mock_repo,
            StubUserRepository,
            mock_dataplane_repo,
            mock_provisioner,
            windows(),
        );

        let mut command = command_for("eu-west", DataPlaneMode::Dedicated);
        command.organisation_id = organisation_id;

        let result = service.create_deployment(command).await;

        match result {
            Err(CoreError::NoDataPlaneAvailable { region, mode }) => {
                assert_eq!(region, "eu-west");
                assert_eq!(mode, "dedicated");
            }
            other => panic!("expected NoDataPlaneAvailable, got {other:?}"),
        }
    }

    fn upgrading_deployment() -> Deployment {
        let mut deployment =
            sample_deployment(DeploymentId(Uuid::new_v4()), OrganisationId(Uuid::new_v4()));
        deployment.status = DeploymentStatus::Upgrading;
        deployment
    }

    /// An upgrade rewrites the instance in place. A delete landing halfway
    /// through leaves resources nobody is tracking, so it waits.
    #[tokio::test]
    async fn a_deployment_being_upgraded_refuses_a_delete() {
        let mut mock_repo = MockDeploymentRepository::new();
        mock_repo
            .expect_get_by_id()
            .returning(|_| Box::pin(async { Ok(Some(upgrading_deployment())) }));
        mock_repo.expect_delete().times(0);

        let service = DeploymentServiceImpl::new(
            mock_repo,
            StubUserRepository,
            MockDataPlaneRepository::new(),
            MockClusterProvisioner::new(),
            windows(),
        );

        let error = service
            .delete_deployment(DeploymentId(Uuid::new_v4()))
            .await
            .expect_err("an upgrade is in flight");

        assert!(matches!(error, CoreError::DeploymentBusy { .. }));
    }

    /// Same rule on the organisation-scoped path, which is the one the API
    /// actually calls.
    #[tokio::test]
    async fn a_deployment_being_upgraded_refuses_a_delete_for_its_organisation() {
        // The same deployment every time, because this path checks ownership
        // before anything else and a fresh organisation id would fail there
        // instead, passing the test for the wrong reason.
        let deployment = upgrading_deployment();
        let organisation_id = deployment.organisation_id;
        let deployment_id = deployment.id;

        let mut mock_repo = MockDeploymentRepository::new();
        mock_repo.expect_get_by_id().returning(move |_| {
            let deployment = deployment.clone();
            Box::pin(async move { Ok(Some(deployment)) })
        });
        mock_repo.expect_delete().times(0);

        let service = DeploymentServiceImpl::new(
            mock_repo,
            StubUserRepository,
            MockDataPlaneRepository::new(),
            MockClusterProvisioner::new(),
            windows(),
        );

        let error = service
            .delete_deployment_for_organisation(organisation_id, deployment_id)
            .await
            .expect_err("an upgrade is in flight");

        assert!(matches!(error, CoreError::DeploymentBusy { .. }));
    }

    /// Resizing mid-upgrade would have the control plane reserve room for a
    /// shape the data plane is not building.
    #[tokio::test]
    async fn a_deployment_being_upgraded_refuses_a_change() {
        let mut mock_repo = MockDeploymentRepository::new();
        mock_repo
            .expect_get_by_id()
            .returning(|_| Box::pin(async { Ok(Some(upgrading_deployment())) }));
        mock_repo.expect_update().times(0);

        let service = DeploymentServiceImpl::new(
            mock_repo,
            StubUserRepository,
            MockDataPlaneRepository::new(),
            MockClusterProvisioner::new(),
            windows(),
        );

        let command =
            UpdateDeploymentCommand::default().with_name(DeploymentName("renamed".to_string()));

        let error = service
            .update_deployment(DeploymentId(Uuid::new_v4()), command)
            .await
            .expect_err("an upgrade is in flight");

        assert!(matches!(error, CoreError::DeploymentBusy { .. }));
    }

    /// The lock is only for an upgrade. Abandoning a deployment that never
    /// came up is a reasonable thing to want, and a tear-down already refuses
    /// a second one on its own.
    #[tokio::test]
    async fn every_other_state_can_still_be_deleted() {
        for status in [
            DeploymentStatus::Pending,
            DeploymentStatus::InProgress,
            DeploymentStatus::Successful,
            DeploymentStatus::Failed,
        ] {
            let mut mock_repo = MockDeploymentRepository::new();
            let held = status.clone();
            mock_repo.expect_get_by_id().returning(move |_| {
                let mut deployment =
                    sample_deployment(DeploymentId(Uuid::new_v4()), OrganisationId(Uuid::new_v4()));
                deployment.status = held.clone();
                Box::pin(async move { Ok(Some(deployment)) })
            });
            mock_repo
                .expect_delete()
                .times(1)
                .returning(|_| Box::pin(async { Ok(()) }));

            let service = DeploymentServiceImpl::new(
                mock_repo,
                StubUserRepository,
                MockDataPlaneRepository::new(),
                MockClusterProvisioner::new(),
                windows(),
            );

            assert!(
                service
                    .delete_deployment(DeploymentId(Uuid::new_v4()))
                    .await
                    .is_ok(),
                "{status:?} must still be deletable"
            );
        }
    }
}
