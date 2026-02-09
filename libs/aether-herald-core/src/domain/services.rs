use crate::domain::entities::dataplane::DataPlaneId;
use crate::domain::entities::deployment::DeploymentId;
use crate::domain::error::HeraldError;
use crate::domain::ports::{ControlPlaneRepository, HeraldService, MessageBusRepository};
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

    struct HeraldServiceTestBuilder {
        control_plane: Arc<MockControlPlaneRepository>,
        message_bus: Arc<MockMessageBusRepository>,
        dataplane_id: DataPlaneId,
    }

    impl HeraldServiceTestBuilder {
        fn new() -> Self {
            Self {
                control_plane: Arc::new(MockControlPlaneRepository::new()),
                message_bus: Arc::new(MockMessageBusRepository::new()),
                dataplane_id: DataPlaneId::new("test-dataplane"),
            }
        }

        fn with_control_plane(mut self, control_plane: MockControlPlaneRepository) -> Self {
            self.control_plane = Arc::new(control_plane);
            self
        }

        fn with_message_bus(mut self, message_bus: MockMessageBusRepository) -> Self {
            self.message_bus = Arc::new(message_bus);
            self
        }

        fn with_dataplane_id(mut self, dataplane_id: DataPlaneId) -> Self {
            self.dataplane_id = dataplane_id;
            self
        }

        fn build(self) -> HeraldServiceImpl<MockControlPlaneRepository, MockMessageBusRepository> {
            HeraldServiceImpl::new(self.control_plane, self.message_bus, self.dataplane_id)
        }
    }

    #[tokio::test]
    async fn test_sync_all_deployments_success() {
        // Arrange
        let deployment1 = Arc::new(create_test_deployment("dep-1", "deployment-one"));
        let deployment2 = Arc::new(create_test_deployment("dep-2", "deployment-two"));

        let action1 = Arc::new(create_test_action("dep-1", "ferriskey", "create"));
        let action2 = Arc::new(create_test_action("dep-1", "ferriskey", "update"));
        let action3 = Arc::new(create_test_action("dep-2", "postgres", "create"));

        let mut mock_control_plane = MockControlPlaneRepository::new();
        let d1 = deployment1.clone();
        let d2 = deployment2.clone();
        mock_control_plane
            .expect_list_deployments()
            .times(1)
            .returning(move |_| {
                let d1 = d1.clone();
                let d2 = d2.clone();
                Box::pin(async move { Ok(vec![(*d1).clone(), (*d2).clone()]) })
            });

        let a1 = action1.clone();
        let a2 = action2.clone();
        mock_control_plane
            .expect_claim_actions()
            .withf(|_, dep_id| dep_id.0 == "dep-1")
            .times(1)
            .returning(move |_, _| {
                let a1 = a1.clone();
                let a2 = a2.clone();
                Box::pin(async move { Ok(vec![(*a1).clone(), (*a2).clone()]) })
            });

        let a3 = action3.clone();
        mock_control_plane
            .expect_claim_actions()
            .withf(|_, dep_id| dep_id.0 == "dep-2")
            .times(1)
            .returning(move |_, _| {
                let a3 = a3.clone();
                Box::pin(async move { Ok(vec![(*a3).clone()]) })
            });

        let mut mock_message_bus = MockMessageBusRepository::new();
        mock_message_bus
            .expect_publish()
            .times(3)
            .returning(|_| Box::pin(async { Ok(()) }));

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

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
            .returning(|_| Box::pin(async { Ok(vec![]) }));

        let mock_message_bus = MockMessageBusRepository::new();

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

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
                Box::pin(async {
                    Err(HeraldError::ControlPlane {
                        message: "Control plane error".to_string(),
                    })
                })
            });

        let mock_message_bus = MockMessageBusRepository::new();

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

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
        let deployment = Arc::new(create_test_deployment("dep-1", "deployment-one"));
        let action = Arc::new(create_test_action("dep-1", "ferriskey", "create"));

        let mut mock_control_plane = MockControlPlaneRepository::new();
        let d = deployment.clone();
        mock_control_plane
            .expect_list_deployments()
            .times(1)
            .returning(move |_| {
                let d = d.clone();
                Box::pin(async move { Ok(vec![(*d).clone()]) })
            });

        let a = action.clone();
        mock_control_plane
            .expect_claim_actions()
            .times(1)
            .returning(move |_, _| {
                let a = a.clone();
                Box::pin(async move { Ok(vec![(*a).clone()]) })
            });

        let mut mock_message_bus = MockMessageBusRepository::new();
        mock_message_bus.expect_publish().times(1).returning(|_| {
            Box::pin(async {
                Err(HeraldError::MessageBus {
                    message: "Message bus error".to_string(),
                })
            })
        });

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

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
        let action1 = Arc::new(create_test_action("dep-1", "ferriskey", "create"));
        let action2 = Arc::new(create_test_action("dep-1", "ferriskey", "update"));

        let mut mock_control_plane = MockControlPlaneRepository::new();
        let a1 = action1.clone();
        let a2 = action2.clone();
        mock_control_plane
            .expect_claim_actions()
            .times(1)
            .returning(move |_, _| {
                let a1 = a1.clone();
                let a2 = a2.clone();
                Box::pin(async move { Ok(vec![(*a1).clone(), (*a2).clone()]) })
            });

        let mut mock_message_bus = MockMessageBusRepository::new();
        mock_message_bus
            .expect_publish()
            .times(2)
            .returning(|_| Box::pin(async { Ok(()) }));

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

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
            .returning(|_, _| Box::pin(async { Ok(vec![]) }));

        let mock_message_bus = MockMessageBusRepository::new();

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

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
                Box::pin(async {
                    Err(HeraldError::ControlPlane {
                        message: "Cannot claim actions".to_string(),
                    })
                })
            });

        let mock_message_bus = MockMessageBusRepository::new();

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

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
        let action = Arc::new(create_test_action("dep-1", "ferriskey", "create"));

        let mut mock_control_plane = MockControlPlaneRepository::new();
        let a = action.clone();
        mock_control_plane
            .expect_claim_actions()
            .times(1)
            .returning(move |_, _| {
                let a = a.clone();
                Box::pin(async move { Ok(vec![(*a).clone()]) })
            });

        let mut mock_message_bus = MockMessageBusRepository::new();
        mock_message_bus.expect_publish().times(1).returning(|_| {
            Box::pin(async {
                Err(HeraldError::MessageBus {
                    message: "Publish failed".to_string(),
                })
            })
        });

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

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
        let _service = HeraldServiceTestBuilder::new()
            .with_dataplane_id(DataPlaneId::new("test-dp-123"))
            .build();
    }
}
