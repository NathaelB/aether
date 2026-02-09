use aether_auth::Identity;

use crate::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        ports::{DataPlaneRepository, DataPlaneService},
        value_objects::{CreateDataplaneCommand, DataPlaneId, ListDataPlaneDeploymentsCommand},
    },
    deployments::{Deployment, DeploymentId, ports::DeploymentRepository},
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
}

impl<DP, D> DataPlaneServiceImpl<DP, D>
where
    DP: DataPlaneRepository,
    D: DeploymentRepository,
{
    pub fn new(dataplane_repository: DP, deployment_repository: D) -> Self {
        Self {
            dataplane_repository,
            deployment_repository,
        }
    }
}

impl<DP, D> DataPlaneService for DataPlaneServiceImpl<DP, D>
where
    DP: DataPlaneRepository,
    D: DeploymentRepository,
{
    async fn create_dataplane(
        &self,
        _identity: Identity,
        command: CreateDataplaneCommand,
    ) -> Result<DataPlane, CoreError> {
        let dataplane = DataPlane::new(command.mode, command.region, command.capacity);
        self.dataplane_repository.save(&dataplane).await?;

        Ok(dataplane)
    }

    async fn list_dataplanes(&self, _identity: Identity) -> Result<Vec<DataPlane>, CoreError> {
        self.dataplane_repository.list_all().await
    }

    async fn get_dataplane(
        &self,
        _identity: Identity,
        dataplane_id: DataPlaneId,
    ) -> Result<DataPlane, CoreError> {
        let dataplane = self
            .dataplane_repository
            .find_by_id(&dataplane_id)
            .await?
            .ok_or(CoreError::DataPlaneNotFound { id: dataplane_id })?;

        Ok(dataplane)
    }

    async fn get_deployments_in_dataplane(
        &self,
        _identity: Identity,
        dataplane_id: DataPlaneId,
        command: ListDataPlaneDeploymentsCommand,
    ) -> Result<Vec<Deployment>, CoreError> {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        dataplane::ports::MockDataPlaneRepository,
        deployments::{
            DeploymentKind, DeploymentName, DeploymentStatus, DeploymentVersion,
            ports::MockDeploymentRepository,
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
            kind: DeploymentKind::Ferriskey,
            version: DeploymentVersion("1.0.0".to_string()),
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

        DataPlaneServiceImpl::new(dataplane_repository, deployment_repository)
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
                Identity::Client(aether_auth::Client {
                    id: "id".to_string(),
                    client_id: "client".to_string(),
                    roles: vec![],
                    scopes: vec![],
                }),
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
                Identity::Client(aether_auth::Client {
                    id: "id".to_string(),
                    client_id: "client".to_string(),
                    roles: vec![],
                    scopes: vec![],
                }),
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
