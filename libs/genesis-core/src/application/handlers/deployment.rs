use std::sync::Arc;

use tracing::info;

use crate::domain::entities::action_event::ActionEvent;
use crate::domain::entities::deployment_payload::DeploymentPayloadV1;
use crate::domain::entities::identity_instance::{DesiredIdentityInstance, IdentityInstanceRef};
use crate::domain::error::GenesisError;
use crate::domain::ports::{BoxFuture, EventHandler, IdentityInstancePort};

/// What a `deployment.<kind>` event should do to the cluster once decoded.
///
/// `create` and `update` both converge to the same desired state: ordering between
/// them is not guaranteed by the message bus, so both simply apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeploymentAction {
    Apply,
    Delete,
}

/// Handles events with routing key `deployment.<kind>` by converging the corresponding
/// `IdentityInstance` custom resource toward the desired state, through the
/// [`IdentityInstancePort`].
pub struct DeploymentEventHandler {
    routing_key: String,
    action: DeploymentAction,
    identity_instances: Arc<dyn IdentityInstancePort>,
}

impl DeploymentEventHandler {
    pub fn create(identity_instances: Arc<dyn IdentityInstancePort>) -> Self {
        Self::with_action("create", DeploymentAction::Apply, identity_instances)
    }

    pub fn update(identity_instances: Arc<dyn IdentityInstancePort>) -> Self {
        Self::with_action("update", DeploymentAction::Apply, identity_instances)
    }

    pub fn delete(identity_instances: Arc<dyn IdentityInstancePort>) -> Self {
        Self::with_action("delete", DeploymentAction::Delete, identity_instances)
    }

    fn with_action(
        kind: &str,
        action: DeploymentAction,
        identity_instances: Arc<dyn IdentityInstancePort>,
    ) -> Self {
        Self {
            routing_key: format!("deployment.{kind}"),
            action,
            identity_instances,
        }
    }
}

impl EventHandler for DeploymentEventHandler {
    fn routing_key(&self) -> &str {
        &self.routing_key
    }

    fn handle<'a>(&'a self, event: ActionEvent) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async move {
            info!(
                action_id = %event.action_id,
                deployment_id = %event.deployment_id,
                routing_key = %event.routing_key,
                "handling deployment event"
            );

            let payload = DeploymentPayloadV1::from_value(&event.payload)?;

            match self.action {
                DeploymentAction::Apply => {
                    let desired = DesiredIdentityInstance::from_payload(&payload)?;
                    self.identity_instances.apply(&desired).await
                }
                DeploymentAction::Delete => {
                    let reference = IdentityInstanceRef::for_deployment(
                        payload.deployment_id,
                        payload.namespace,
                    );
                    self.identity_instances.delete(&reference).await
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::dispatcher::EventDispatcher;
    use crate::domain::entities::identity_instance::IdentityInstanceProvider;
    use chrono::Utc;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use uuid::Uuid;

    /// Hand-rolled test double standing in for a real Kubernetes cluster. `state`
    /// models server-side apply: applying the same reference twice overwrites the same
    /// entry rather than producing two resources.
    #[derive(Default)]
    struct FakeIdentityInstancePort {
        state: Mutex<HashMap<IdentityInstanceRef, DesiredIdentityInstance>>,
        apply_calls: Mutex<Vec<DesiredIdentityInstance>>,
        delete_calls: Mutex<Vec<IdentityInstanceRef>>,
    }

    impl IdentityInstancePort for FakeIdentityInstancePort {
        fn apply<'a>(
            &'a self,
            desired: &'a DesiredIdentityInstance,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            Box::pin(async move {
                self.state
                    .lock()
                    .unwrap()
                    .insert(desired.reference.clone(), desired.clone());
                self.apply_calls.lock().unwrap().push(desired.clone());
                Ok(())
            })
        }

        fn delete<'a>(
            &'a self,
            reference: &'a IdentityInstanceRef,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            Box::pin(async move {
                // Idempotent: removing an absent key is still `Ok`.
                self.state.lock().unwrap().remove(reference);
                self.delete_calls.lock().unwrap().push(reference.clone());
                Ok(())
            })
        }
    }

    const DEPLOYMENT_ID: &str = "b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d";
    const DATAPLANE_ID: &str = "1c2b3a4d-5e6f-4a7b-8c9d-0e1f2a3b4c5e";
    const ORG_ID: &str = "9f8e7d6c-5b4a-3c2d-1e0f-a1b2c3d4e5f6";
    const CREATED_BY: &str = "11111111-2222-3333-4444-555555555555";

    fn deployment_event(routing_key: &str, kind: &str, version: &str) -> ActionEvent {
        ActionEvent {
            action_id: Uuid::new_v4(),
            deployment_id: Uuid::parse_str(DEPLOYMENT_ID).unwrap(),
            dataplane_id: Uuid::parse_str(DATAPLANE_ID).unwrap(),
            routing_key: routing_key.to_string(),
            version: 1,
            payload: json!({
                "deployment_id": DEPLOYMENT_ID,
                "dataplane_id": DATAPLANE_ID,
                "organisation_id": ORG_ID,
                "name": "acme-prod",
                "kind": kind,
                "version": version,
                "namespace": "aether-acme-prod",
                "created_by": CREATED_BY,
            }),
            occurred_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn create_applies_the_expected_desired_state() {
        let port = Arc::new(FakeIdentityInstancePort::default());
        let handler = DeploymentEventHandler::create(port.clone());

        handler
            .handle(deployment_event("deployment.create", "keycloak", "25.0.0"))
            .await
            .expect("handled");

        let calls = port.apply_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].reference.name,
            "deployment-b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d"
        );
        assert_eq!(calls[0].reference.namespace, "aether-acme-prod");
        assert_eq!(calls[0].provider, IdentityInstanceProvider::Keycloak);
        assert_eq!(calls[0].version, "25.0.0");
        assert!(port.delete_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn update_applies_the_expected_desired_state() {
        let port = Arc::new(FakeIdentityInstancePort::default());
        let handler = DeploymentEventHandler::update(port.clone());

        handler
            .handle(deployment_event("deployment.update", "ferriskey", "0.6.0"))
            .await
            .expect("handled");

        let calls = port.apply_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].provider, IdentityInstanceProvider::Ferriskey);
        assert_eq!(calls[0].version, "0.6.0");
    }

    #[tokio::test]
    async fn delete_targets_the_same_reference_create_would_have_used() {
        let port = Arc::new(FakeIdentityInstancePort::default());
        let create_handler = DeploymentEventHandler::create(port.clone());
        let delete_handler = DeploymentEventHandler::delete(port.clone());

        create_handler
            .handle(deployment_event("deployment.create", "keycloak", "25.0.0"))
            .await
            .expect("handled");
        delete_handler
            .handle(deployment_event("deployment.delete", "keycloak", "25.0.0"))
            .await
            .expect("handled");

        assert_eq!(port.delete_calls.lock().unwrap().len(), 1);
        assert!(port.state.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn deleting_an_already_absent_resource_succeeds() {
        let port = Arc::new(FakeIdentityInstancePort::default());
        let handler = DeploymentEventHandler::delete(port.clone());

        let result = handler
            .handle(deployment_event("deployment.delete", "keycloak", "25.0.0"))
            .await;

        assert!(result.is_ok());
    }

    /// Mandatory idempotency test: redelivering the identical event must converge on a
    /// single resource and a single (repeatable) apply outcome, never fail on the
    /// second delivery.
    #[tokio::test]
    async fn dispatching_the_identical_event_twice_converges_on_one_resource() {
        let port = Arc::new(FakeIdentityInstancePort::default());
        let handler = DeploymentEventHandler::create(port.clone());
        let event = deployment_event("deployment.create", "keycloak", "25.0.0");

        handler
            .handle(event.clone())
            .await
            .expect("first delivery handled");
        handler.handle(event).await.expect("redelivery handled");

        let calls = port.apply_calls.lock().unwrap();
        assert_eq!(calls.len(), 2, "handler runs for each delivery");
        assert_eq!(
            calls[0], calls[1],
            "both deliveries produce the same outcome"
        );

        let state = port.state.lock().unwrap();
        assert_eq!(
            state.len(),
            1,
            "at most one resource exists after redelivery"
        );
    }

    #[tokio::test]
    async fn unknown_routing_key_is_acked_not_retried() {
        let port = Arc::new(FakeIdentityInstancePort::default());
        let handlers: Vec<Arc<dyn EventHandler>> = vec![
            Arc::new(DeploymentEventHandler::create(port.clone())),
            Arc::new(DeploymentEventHandler::update(port.clone())),
            Arc::new(DeploymentEventHandler::delete(port.clone())),
        ];
        let dispatcher = EventDispatcher::new(handlers);

        let result = dispatcher
            .dispatch(deployment_event("deployment.rename", "keycloak", "25.0.0"))
            .await;

        // Ok(()) is what causes the consumer to ack rather than nack-and-requeue.
        assert!(result.is_ok());
        assert!(port.apply_calls.lock().unwrap().is_empty());
        assert!(port.delete_calls.lock().unwrap().is_empty());
    }
}
