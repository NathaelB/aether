use crate::domain::entities::action::{AckFailure, ActionEvent, ActionFailureReason};
use crate::domain::entities::dataplane::DataPlaneId;
use crate::domain::entities::deployment::DeploymentId;
use crate::domain::entities::shard::ShardConfig;
use crate::domain::error::HeraldError;
use crate::domain::ports::{ControlPlaneRepository, HeraldService, MessageBusRepository};
use std::sync::Arc;
use tracing::warn;

pub struct HeraldServiceImpl<CP, MB>
where
    CP: ControlPlaneRepository,
    MB: MessageBusRepository,
{
    control_plane: Arc<CP>,
    message_bus: Arc<MB>,
    dataplane_id: DataPlaneId,
    shard_config: ShardConfig,
}

impl<CP, MB> HeraldServiceImpl<CP, MB>
where
    CP: ControlPlaneRepository,
    MB: MessageBusRepository,
{
    pub fn new(
        control_plane: Arc<CP>,
        message_bus: Arc<MB>,
        dataplane_id: DataPlaneId,
        shard_config: ShardConfig,
    ) -> Self {
        Self {
            control_plane,
            message_bus,
            dataplane_id,
            shard_config,
        }
    }

    /// Claims pending actions for a single deployment, publishes each to the
    /// message bus, and acknowledges the whole batch with the control plane
    /// in a single call.
    ///
    /// A per-action publish failure does not abort the batch: it is recorded
    /// in the ack's `failed` list (with `PublishFailed`) instead, so the
    /// control plane can move that action to a terminal `Failed` state
    /// rather than leaving it leased until it silently expires and gets
    /// republished forever. This is the mechanism C3's "publish first, then
    /// ack" delivery semantics depend on.
    ///
    /// Acking is best-effort: a failure to ack is logged and swallowed,
    /// never propagated. The control plane's ack is idempotent (it only
    /// transitions actions that are still leased), so a lost ack is safe by
    /// design — the lease simply expires and the action is reclaimed.
    async fn claim_publish_and_ack(&self, deployment_id: &DeploymentId) -> Result<(), HeraldError> {
        let actions = self
            .control_plane
            .claim_actions(&self.dataplane_id, deployment_id)
            .await?;

        if actions.is_empty() {
            return Ok(());
        }

        let mut published = Vec::with_capacity(actions.len());
        let mut failed = Vec::new();

        for action in actions {
            let action_id = action.id;

            let event: ActionEvent = match action.try_into() {
                Ok(event) => event,
                Err(err) => {
                    warn!(
                        %deployment_id, %action_id, error = %err,
                        "invalid action, marking as failed"
                    );
                    failed.push(AckFailure {
                        action_id,
                        reason: ActionFailureReason::InvalidPayload,
                    });
                    continue;
                }
            };

            match self.message_bus.publish(event).await {
                Ok(()) => published.push(action_id),
                Err(err) => {
                    warn!(
                        %deployment_id, %action_id, error = %err,
                        "failed to publish action event"
                    );
                    failed.push(AckFailure {
                        action_id,
                        reason: ActionFailureReason::PublishFailed,
                    });
                }
            }
        }

        if let Err(err) = self
            .control_plane
            .ack_actions(&self.dataplane_id, deployment_id, published, failed)
            .await
        {
            warn!(
                %deployment_id, error = %err,
                "failed to ack claimed actions; unacked actions will be reclaimed once their lease expires"
            );
        }

        Ok(())
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
            if !self.shard_config.owns_deployment(&deployment.id) {
                continue;
            }

            self.claim_publish_and_ack(&deployment.id).await?;
        }

        Ok(())
    }

    async fn process_deployment(&self, deployment_id: &DeploymentId) -> Result<(), HeraldError> {
        if !self.shard_config.owns_deployment(deployment_id) {
            return Ok(());
        }

        self.claim_publish_and_ack(deployment_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::action::{AckOutcome, Action, ActionId};
    use crate::domain::entities::deployment::Deployment;
    use crate::domain::ports::{MockControlPlaneRepository, MockMessageBusRepository};
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    fn create_test_deployment(id: &str, name: &str) -> Deployment {
        Deployment {
            id: DeploymentId::new(id),
            dataplane_id: DataPlaneId::new("cccccccc-cccc-cccc-cccc-cccccccccccc"),
            name: name.to_string(),
        }
    }

    fn create_test_action(deployment_id: &str, action_type: &str) -> Action {
        Action {
            id: ActionId(Uuid::new_v4()),
            deployment_id: DeploymentId::new(deployment_id),
            dataplane_id: DataPlaneId::new("cccccccc-cccc-cccc-cccc-cccccccccccc"),
            action_type: action_type.to_string(),
            payload: json!({"key": "value"}),
            version: 1,
            occurred_at: Utc::now(),
        }
    }

    struct HeraldServiceTestBuilder {
        control_plane: Arc<MockControlPlaneRepository>,
        message_bus: Arc<MockMessageBusRepository>,
        dataplane_id: DataPlaneId,
        shard_config: ShardConfig,
    }

    impl HeraldServiceTestBuilder {
        fn new() -> Self {
            Self {
                control_plane: Arc::new(MockControlPlaneRepository::new()),
                message_bus: Arc::new(MockMessageBusRepository::new()),
                dataplane_id: DataPlaneId::new("cccccccc-cccc-cccc-cccc-cccccccccccc"),
                // Owns every deployment by default so existing flow
                // assertions are unaffected by sharding.
                shard_config: ShardConfig::new(0, 1),
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
            HeraldServiceImpl::new(
                self.control_plane,
                self.message_bus,
                self.dataplane_id,
                self.shard_config,
            )
        }
    }

    #[tokio::test]
    async fn test_sync_all_deployments_success() {
        // Arrange
        let deployment1 = Arc::new(create_test_deployment(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "deployment-one",
        ));
        let deployment2 = Arc::new(create_test_deployment(
            "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
            "deployment-two",
        ));

        let action1 = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.create",
        ));
        let action2 = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.update",
        ));
        let action3 = Arc::new(create_test_action(
            "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
            "postgres.create",
        ));

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
            .withf(|_, dep_id| dep_id.0 == "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")
            .times(1)
            .returning(move |_, _| {
                let a1 = a1.clone();
                let a2 = a2.clone();
                Box::pin(async move { Ok(vec![(*a1).clone(), (*a2).clone()]) })
            });

        let a3 = action3.clone();
        mock_control_plane
            .expect_claim_actions()
            .withf(|_, dep_id| dep_id.0 == "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb")
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

        mock_control_plane
            .expect_ack_actions()
            .withf(|_, dep_id, published, failed| {
                dep_id.0 == "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"
                    && published.len() == 2
                    && failed.is_empty()
            })
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(AckOutcome { acknowledged: 2 }) }));

        mock_control_plane
            .expect_ack_actions()
            .withf(|_, dep_id, published, failed| {
                dep_id.0 == "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"
                    && published.len() == 1
                    && failed.is_empty()
            })
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(AckOutcome { acknowledged: 1 }) }));

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

        // Act
        let result = service.sync_all_deployments().await;

        // Assert: every action published, and each deployment's batch was
        // acked in a single call listing all of its actions as published.
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
    async fn test_sync_all_deployments_partial_publish_failure_acks_one_batch() {
        // Arrange: two actions for the same deployment, one publishes fine,
        // the other fails. A publish failure must not abort the batch nor
        // the sync: both outcomes are reported in a single ack call.
        let deployment = Arc::new(create_test_deployment(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "deployment-one",
        ));
        let ok_action = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.create",
        ));
        let failing_action = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.update",
        ));
        let ok_id = ok_action.id;
        let failing_id = failing_action.id;

        let mut mock_control_plane = MockControlPlaneRepository::new();
        let d = deployment.clone();
        mock_control_plane
            .expect_list_deployments()
            .times(1)
            .returning(move |_| {
                let d = d.clone();
                Box::pin(async move { Ok(vec![(*d).clone()]) })
            });

        let a1 = ok_action.clone();
        let a2 = failing_action.clone();
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
            .withf(move |event| event.action_id == ok_id.0)
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        mock_message_bus
            .expect_publish()
            .withf(move |event| event.action_id == failing_id.0)
            .times(1)
            .returning(|_| {
                Box::pin(async {
                    Err(HeraldError::MessageBus {
                        message: "Message bus error".to_string(),
                    })
                })
            });

        mock_control_plane
            .expect_ack_actions()
            .withf(move |_, _, published, failed| {
                published == &vec![ok_id]
                    && failed
                        == &vec![AckFailure {
                            action_id: failing_id,
                            reason: ActionFailureReason::PublishFailed,
                        }]
            })
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(AckOutcome { acknowledged: 1 }) }));

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

        // Act
        let result = service.sync_all_deployments().await;

        // Assert: the publish failure was recorded via the ack, not
        // propagated, so the sync itself still succeeds.
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_sync_all_deployments_ack_failure_does_not_fail_sync() {
        // Arrange: publish succeeds but the control plane's ack call fails.
        // The publish must not be lost or retried, and the sync must not
        // fail — the ack is best-effort, the control plane's ack is
        // idempotent, and a lost ack just means the lease expires later.
        let deployment = Arc::new(create_test_deployment(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "deployment-one",
        ));
        let action = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.create",
        ));

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

        mock_control_plane
            .expect_ack_actions()
            .times(1)
            .returning(|_, _, _, _| {
                Box::pin(async {
                    Err(HeraldError::ControlPlane {
                        message: "ack endpoint unreachable".to_string(),
                    })
                })
            });

        let mut mock_message_bus = MockMessageBusRepository::new();
        mock_message_bus
            .expect_publish()
            .times(1)
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
    async fn test_process_deployment_success() {
        // Arrange
        let deployment_id = DeploymentId::new("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let action1 = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.create",
        ));
        let action2 = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.update",
        ));

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

        mock_control_plane
            .expect_ack_actions()
            .withf(|_, _, published, failed| published.len() == 2 && failed.is_empty())
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(AckOutcome { acknowledged: 2 }) }));

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

        // Act
        let result = service.process_deployment(&deployment_id).await;

        // Assert: both actions published, then acked together in one call.
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_process_deployment_no_actions() {
        // Arrange
        let deployment_id = DeploymentId::new("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");

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
        let deployment_id = DeploymentId::new("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");

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
    async fn test_process_deployment_partial_publish_failure_acks_one_batch() {
        // Arrange: one action publishes fine, the other fails. Both
        // outcomes must be reported in a single ack call, and the failure
        // must not abort processing of the successful one.
        let deployment_id = DeploymentId::new("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let ok_action = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.create",
        ));
        let failing_action = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.update",
        ));
        let ok_id = ok_action.id;
        let failing_id = failing_action.id;

        let mut mock_control_plane = MockControlPlaneRepository::new();
        let a1 = ok_action.clone();
        let a2 = failing_action.clone();
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
            .withf(move |event| event.action_id == ok_id.0)
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        mock_message_bus
            .expect_publish()
            .withf(move |event| event.action_id == failing_id.0)
            .times(1)
            .returning(|_| {
                Box::pin(async {
                    Err(HeraldError::MessageBus {
                        message: "Publish failed".to_string(),
                    })
                })
            });

        mock_control_plane
            .expect_ack_actions()
            .withf(move |_, _, published, failed| {
                published == &vec![ok_id]
                    && failed
                        == &vec![AckFailure {
                            action_id: failing_id,
                            reason: ActionFailureReason::PublishFailed,
                        }]
            })
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(AckOutcome { acknowledged: 1 }) }));

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

        // Act
        let result = service.process_deployment(&deployment_id).await;

        // Assert: the publish failure was recorded via the ack, not
        // propagated.
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_process_deployment_ack_failure_does_not_fail_processing() {
        // Arrange: publish succeeds but acking fails. The failure must be
        // swallowed (logged, not propagated) rather than reported to the
        // caller or panicking.
        let deployment_id = DeploymentId::new("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let action = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.create",
        ));

        let mut mock_control_plane = MockControlPlaneRepository::new();
        let a = action.clone();
        mock_control_plane
            .expect_claim_actions()
            .times(1)
            .returning(move |_, _| {
                let a = a.clone();
                Box::pin(async move { Ok(vec![(*a).clone()]) })
            });

        mock_control_plane
            .expect_ack_actions()
            .times(1)
            .returning(|_, _, _, _| {
                Box::pin(async {
                    Err(HeraldError::ControlPlane {
                        message: "ack endpoint unreachable".to_string(),
                    })
                })
            });

        let mut mock_message_bus = MockMessageBusRepository::new();
        mock_message_bus
            .expect_publish()
            .times(1)
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

    #[test]
    fn test_herald_service_impl_creation() {
        let _service = HeraldServiceTestBuilder::new()
            .with_dataplane_id(DataPlaneId::new("test-dp-123"))
            .build();
    }
}
