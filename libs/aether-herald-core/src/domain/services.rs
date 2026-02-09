use crate::domain::entities::dataplane::DataPlaneId;
use crate::domain::entities::deployment::DeploymentId;
use crate::domain::error::HeraldError;
use crate::domain::ports::{ControlPlaneRepository, HeraldService, MessageBusRepository};
use async_trait::async_trait;
use std::sync::Arc;

pub struct HeraldServiceImpl<CP, MB>
where
    CP: ControlPlaneRepository,
    MB: MessageBusRepository,
{
    control_plane: Arc<CP>,
    message_bus: Arc<MB>,
    dataplane_id: DataPlaneId,
}

impl<CP, MB> HeraldServiceImpl<CP, MB>
where
    CP: ControlPlaneRepository,
    MB: MessageBusRepository,
{
    pub fn new(control_plane: Arc<CP>, message_bus: Arc<MB>, dataplane_id: DataPlaneId) -> Self {
        Self {
            control_plane,
            message_bus,
            dataplane_id,
        }
    }
}

#[async_trait]
impl<CP, MB> HeraldService for HeraldServiceImpl<CP, MB>
where
    CP: ControlPlaneRepository,
    MB: MessageBusRepository,
{
    async fn sync_all_deployments(&self) -> Result<(), HeraldError> {
        let deployments = self
            .control_plane
            .list_deployments(&self.dataplane_id)
            .await?;

        for deployment in deployments {
            let actions = self
                .control_plane
                .claim_actions(&self.dataplane_id, &deployment.id)
                .await?;

            for action in actions {
                let event = action.into();
                self.message_bus.publish(event).await?;
            }
        }

        Ok(())
    }

    async fn process_deployment(&self, deployment_id: &DeploymentId) -> Result<(), HeraldError> {
        let actions = self
            .control_plane
            .claim_actions(&self.dataplane_id, deployment_id)
            .await?;

        for action in actions {
            let event = action.into();
            self.message_bus.publish(event).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::action::{Action, ActionId};
    use crate::domain::entities::deployment::Deployment;
    use crate::domain::ports::{MockControlPlaneRepository, MockMessageBusRepository};
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    fn create_test_deployment(id: &str, name: &str) -> Deployment {
        Deployment {
            id: DeploymentId::new(id),
            dataplane_id: DataPlaneId::new("test-dataplane"),
            name: name.to_string(),
        }
    }

    fn create_test_action(deployment_id: &str, resource: &str, kind: &str) -> Action {
        Action {
            id: ActionId(Uuid::new_v4()),
            deployment_id: DeploymentId::new(deployment_id),
            resource: resource.to_string(),
            kind: kind.to_string(),
            payload: json!({"key": "value"}),
            occurred_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_sync_all_deployments_success() {
        // Arrange
        let deployment1 = create_test_deployment("dep-1", "deployment-one");
        let deployment2 = create_test_deployment("dep-2", "deployment-two");

        let action1 = create_test_action("dep-1", "ferriskey", "create");
        let action2 = create_test_action("dep-1", "ferriskey", "update");
        let action3 = create_test_action("dep-2", "postgres", "create");

        let mut mock_control_plane = MockControlPlaneRepository::new();
        mock_control_plane
            .expect_list_deployments()
            .times(1)
            .returning(move |_| Ok(vec![deployment1.clone(), deployment2.clone()]));

        mock_control_plane
            .expect_claim_actions()
            .withf(|_, dep_id| dep_id.0 == "dep-1")
            .times(1)
            .returning(move |_, _| Ok(vec![action1.clone(), action2.clone()]));

        mock_control_plane
            .expect_claim_actions()
            .withf(|_, dep_id| dep_id.0 == "dep-2")
            .times(1)
            .returning(move |_, _| Ok(vec![action3.clone()]));

        let mut mock_message_bus = MockMessageBusRepository::new();
        mock_message_bus
            .expect_publish()
            .times(3) // 2 actions for dep-1 + 1 action for dep-2
            .returning(|_| Ok(()));

        let service = HeraldServiceImpl::new(
            Arc::new(mock_control_plane),
            Arc::new(mock_message_bus),
            DataPlaneId::new("test-dataplane"),
        );

        // Act
        let result = service.sync_all_deployments().await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_sync_all_deployments_no_deployments() {
        // Arrange
        let mut mock_control_plane = MockControlPlaneRepository::new();
        mock_control_plane
            .expect_list_deployments()
            .times(1)
            .returning(|_| Ok(vec![]));

        let mock_message_bus = MockMessageBusRepository::new();
        // No publish calls expected

        let service = HeraldServiceImpl::new(
            Arc::new(mock_control_plane),
            Arc::new(mock_message_bus),
            DataPlaneId::new("test-dataplane"),
        );

        // Act
        let result = service.sync_all_deployments().await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_sync_all_deployments_control_plane_error() {
        // Arrange
        let mut mock_control_plane = MockControlPlaneRepository::new();
        mock_control_plane
            .expect_list_deployments()
            .times(1)
            .returning(|_| {
                Err(HeraldError::ControlPlane {
                    message: "Control plane error".to_string(),
                })
            });

        let mock_message_bus = MockMessageBusRepository::new();

        let service = HeraldServiceImpl::new(
            Arc::new(mock_control_plane),
            Arc::new(mock_message_bus),
            DataPlaneId::new("test-dataplane"),
        );

        // Act
        let result = service.sync_all_deployments().await;

        // Assert
        assert!(result.is_err());
        if let Err(HeraldError::ControlPlane { message }) = result {
            assert_eq!(message, "Control plane error");
        } else {
            panic!("Expected ControlPlane error");
        }
    }

    #[tokio::test]
    async fn test_sync_all_deployments_message_bus_error() {
        // Arrange
        let deployment = create_test_deployment("dep-1", "deployment-one");
        let action = create_test_action("dep-1", "ferriskey", "create");

        let mut mock_control_plane = MockControlPlaneRepository::new();
        mock_control_plane
            .expect_list_deployments()
            .times(1)
            .returning(move |_| Ok(vec![deployment.clone()]));

        mock_control_plane
            .expect_claim_actions()
            .times(1)
            .returning(move |_, _| Ok(vec![action.clone()]));

        let mut mock_message_bus = MockMessageBusRepository::new();
        mock_message_bus.expect_publish().times(1).returning(|_| {
            Err(HeraldError::MessageBus {
                message: "Message bus error".to_string(),
            })
        });

        let service = HeraldServiceImpl::new(
            Arc::new(mock_control_plane),
            Arc::new(mock_message_bus),
            DataPlaneId::new("test-dataplane"),
        );

        // Act
        let result = service.sync_all_deployments().await;

        // Assert
        assert!(result.is_err());
        if let Err(HeraldError::MessageBus { message }) = result {
            assert_eq!(message, "Message bus error");
        } else {
            panic!("Expected MessageBus error");
        }
    }

    #[tokio::test]
    async fn test_process_deployment_success() {
        // Arrange
        let deployment_id = DeploymentId::new("dep-1");
        let action1 = create_test_action("dep-1", "ferriskey", "create");
        let action2 = create_test_action("dep-1", "ferriskey", "update");

        let mut mock_control_plane = MockControlPlaneRepository::new();
        mock_control_plane
            .expect_claim_actions()
            .times(1)
            .returning(move |_, _| Ok(vec![action1.clone(), action2.clone()]));

        let mut mock_message_bus = MockMessageBusRepository::new();
        mock_message_bus
            .expect_publish()
            .times(2)
            .returning(|_| Ok(()));

        let service = HeraldServiceImpl::new(
            Arc::new(mock_control_plane),
            Arc::new(mock_message_bus),
            DataPlaneId::new("test-dataplane"),
        );

        // Act
        let result = service.process_deployment(&deployment_id).await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_process_deployment_no_actions() {
        // Arrange
        let deployment_id = DeploymentId::new("dep-1");

        let mut mock_control_plane = MockControlPlaneRepository::new();
        mock_control_plane
            .expect_claim_actions()
            .times(1)
            .returning(|_, _| Ok(vec![]));

        let mock_message_bus = MockMessageBusRepository::new();
        // No publish calls expected

        let service = HeraldServiceImpl::new(
            Arc::new(mock_control_plane),
            Arc::new(mock_message_bus),
            DataPlaneId::new("test-dataplane"),
        );

        // Act
        let result = service.process_deployment(&deployment_id).await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_process_deployment_claim_actions_error() {
        // Arrange
        let deployment_id = DeploymentId::new("dep-1");

        let mut mock_control_plane = MockControlPlaneRepository::new();
        mock_control_plane
            .expect_claim_actions()
            .times(1)
            .returning(|_, _| {
                Err(HeraldError::ControlPlane {
                    message: "Cannot claim actions".to_string(),
                })
            });

        let mock_message_bus = MockMessageBusRepository::new();

        let service = HeraldServiceImpl::new(
            Arc::new(mock_control_plane),
            Arc::new(mock_message_bus),
            DataPlaneId::new("test-dataplane"),
        );

        // Act
        let result = service.process_deployment(&deployment_id).await;

        // Assert
        assert!(result.is_err());
        if let Err(HeraldError::ControlPlane { message }) = result {
            assert_eq!(message, "Cannot claim actions");
        } else {
            panic!("Expected ControlPlane error");
        }
    }

    #[tokio::test]
    async fn test_process_deployment_publish_error() {
        // Arrange
        let deployment_id = DeploymentId::new("dep-1");
        let action = create_test_action("dep-1", "ferriskey", "create");

        let mut mock_control_plane = MockControlPlaneRepository::new();
        mock_control_plane
            .expect_claim_actions()
            .times(1)
            .returning(move |_, _| Ok(vec![action.clone()]));

        let mut mock_message_bus = MockMessageBusRepository::new();
        mock_message_bus.expect_publish().times(1).returning(|_| {
            Err(HeraldError::MessageBus {
                message: "Publish failed".to_string(),
            })
        });

        let service = HeraldServiceImpl::new(
            Arc::new(mock_control_plane),
            Arc::new(mock_message_bus),
            DataPlaneId::new("test-dataplane"),
        );

        // Act
        let result = service.process_deployment(&deployment_id).await;

        // Assert
        assert!(result.is_err());
        if let Err(HeraldError::MessageBus { message }) = result {
            assert_eq!(message, "Publish failed");
        } else {
            panic!("Expected MessageBus error");
        }
    }

    #[test]
    fn test_herald_service_impl_creation() {
        let mock_control_plane = MockControlPlaneRepository::new();
        let mock_message_bus = MockMessageBusRepository::new();
        let dataplane_id = DataPlaneId::new("test-dp-123");

        let _service = HeraldServiceImpl::new(
            Arc::new(mock_control_plane),
            Arc::new(mock_message_bus),
            dataplane_id,
        );
    }
}
