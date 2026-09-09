use crate::{
    CoreError,
    dataplane::{ports::DataPlaneRepository, value_objects::PlacementPolicy},
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

#[derive(Debug)]
pub struct DeploymentServiceImpl<D, U, DP>
where
    D: DeploymentRepository,
    U: UserRepository,
    DP: DataPlaneRepository,
{
    deployment_repository: D,
    user_repository: U,
    dataplane_repository: DP,
    heartbeat_window: Duration,
}

impl<D, U, DP> DeploymentServiceImpl<D, U, DP>
where
    D: DeploymentRepository,
    U: UserRepository,
    DP: DataPlaneRepository,
{
    pub fn new(
        deployment_repository: D,
        user_repository: U,
        dataplane_repository: DP,
        heartbeat_window: Duration,
    ) -> Self {
        Self {
            deployment_repository,
            user_repository,
            dataplane_repository,
            heartbeat_window,
        }
    }
}

impl<D, U, DP> DeploymentService for DeploymentServiceImpl<D, U, DP>
where
    D: DeploymentRepository,
    U: UserRepository,
    DP: DataPlaneRepository,
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

        let dataplane = self
            .dataplane_repository
            .find_available(
                Some(command.region.clone()),
                command.mode,
                command.resources,
                // Spreading, as it has always done -- now stated rather than
                // buried in an ORDER BY. Switching to packing is one value.
                PlacementPolicy::default(),
                Utc::now() - self.heartbeat_window,
            )
            .await?;

        // Placement failed for one of two reasons the caller acts on
        // differently, so the answer is only computed once it has actually
        // failed -- the happy path pays nothing for the distinction.
        let dataplane = match dataplane {
            Some(dataplane) => dataplane,
            None => {
                let region = command.region.as_str().to_string();

                return Err(
                    if self
                        .dataplane_repository
                        .region_is_served(&command.region)
                        .await?
                    {
                        error!(%region, mode = ?command.mode, "no data plane with room");
                        CoreError::NoDataPlaneAvailable {
                            region,
                            mode: format!("{:?}", command.mode).to_lowercase(),
                        }
                    } else {
                        error!(%region, "region is not served");
                        CoreError::UnknownRegion { region }
                    },
                );
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
            .ok_or(CoreError::InternalError("Deployment not found".to_string()))?;

        if deployment.organisation_id != organisation_id {
            return Err(CoreError::InternalError("Deployment not found".to_string()));
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
            .ok_or(CoreError::InternalError("Deployment not found".to_string()))?;

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

    async fn delete_deployment(&self, deployment_id: DeploymentId) -> Result<(), CoreError> {
        self.deployment_repository.delete(deployment_id).await
    }

    async fn delete_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<(), CoreError> {
        let deployment = self
            .get_deployment_for_organisation(organisation_id, deployment_id)
            .await?;

        self.deployment_repository.delete(deployment.id).await
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
            value_objects::{Capacity, DataPlaneId, DataPlaneMode, DataPlaneStatus, Region},
        },
        deployments::ports::MockDeploymentRepository,
        deployments::{DeploymentKind, DeploymentName, DeploymentStatus, DeploymentVersion},
        user::UserId,
    };
    use chrono::Utc;
    use uuid::Uuid;

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
            version: DeploymentVersion("1.0.0".to_string()),
            status: DeploymentStatus::Pending,
            namespace: "default".to_string(),
            resources: DeploymentResources::DEFAULT,
            created_by: UserId(Uuid::new_v4()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deployed_at: None,
            deleted_at: None,
        }
    }

    fn sample_dataplane() -> DataPlane {
        DataPlane {
            id: DataPlaneId(Uuid::new_v4()),
            mode: DataPlaneMode::Shared,
            region: Region::new("local"),
            status: DataPlaneStatus::Active,
            capacity: Capacity::new(5000, 10240, 10).unwrap(),
            last_seen_at: Some(Utc::now()),
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
            .returning(|_, _, _, _, _| {
                let dataplane = sample_dataplane();
                Box::pin(async move { Ok(Some(dataplane)) })
            });

        let service = DeploymentServiceImpl::new(
            mock_repo,
            StubUserRepository,
            mock_dataplane_repo,
            Duration::seconds(90),
        );
        let command = CreateDeploymentCommand::new(
            OrganisationId(Uuid::new_v4()),
            DeploymentName("app".to_string()),
            DeploymentKind::Keycloak,
            DeploymentVersion("1.0.0".to_string()),
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

    #[tokio::test]
    async fn get_deployment_for_organisation_rejects_mismatch() {
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
            Duration::seconds(90),
        );
        let result = service
            .get_deployment_for_organisation(organisation_id, deployment_id)
            .await;

        assert!(matches!(result, Err(CoreError::InternalError(_))));
    }

    #[tokio::test]
    async fn update_deployment_rejects_empty_command() {
        let service = DeploymentServiceImpl::new(
            MockDeploymentRepository::new(),
            StubUserRepository,
            MockDataPlaneRepository::new(),
            Duration::seconds(90),
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
            Duration::seconds(90),
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
            Duration::seconds(90),
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
            DeploymentVersion("1.0.0".to_string()),
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
    /// jurisdiction nobody chose.
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
            .withf(|region, mode, _, _, _| {
                region.as_ref().map(|r| r.as_str()) == Some("eu-west")
                    && *mode == DataPlaneMode::Dedicated
            })
            .returning(|_, _, _, _, _| {
                let dataplane = sample_dataplane();
                Box::pin(async move { Ok(Some(dataplane)) })
            });

        let service = DeploymentServiceImpl::new(
            mock_repo,
            StubUserRepository,
            mock_dataplane_repo,
            Duration::seconds(90),
        );

        let result = service
            .create_deployment(command_for("eu-west", DataPlaneMode::Dedicated))
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
            .returning(|_, _, _, _, _| Box::pin(async { Ok(None) }));
        mock_dataplane_repo
            .expect_region_is_served()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));

        let service = DeploymentServiceImpl::new(
            MockDeploymentRepository::new(),
            StubUserRepository,
            mock_dataplane_repo,
            Duration::seconds(90),
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
            .returning(|_, _, _, _, _| Box::pin(async { Ok(None) }));
        mock_dataplane_repo
            .expect_region_is_served()
            .times(1)
            .withf(|region| region.as_str() == "antarctica")
            .returning(|_| Box::pin(async { Ok(false) }));

        let service = DeploymentServiceImpl::new(
            MockDeploymentRepository::new(),
            StubUserRepository,
            mock_dataplane_repo,
            Duration::seconds(90),
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
        mock_dataplane_repo
            .expect_find_available()
            .returning(|_, _, _, _, _| {
                let dataplane = sample_dataplane();
                Box::pin(async move { Ok(Some(dataplane)) })
            });
        mock_dataplane_repo.expect_region_is_served().never();

        let service = DeploymentServiceImpl::new(
            mock_repo,
            StubUserRepository,
            mock_dataplane_repo,
            Duration::seconds(90),
        );

        assert!(
            service
                .create_deployment(command_for("fr-par", DataPlaneMode::Shared))
                .await
                .is_ok()
        );
    }
}
