//! Turning a `deployment.network_access` action into the field the operator
//! reconciles.
//!
//! The action carries the whole rule every time, so this writes a state
//! rather than applying a change. A redelivery therefore costs nothing: the
//! second write says the same thing as the first.

use std::sync::Arc;

use tracing::info;

use crate::domain::entities::action_event::ActionEvent;
use crate::domain::entities::identity_instance::IdentityInstanceRef;
use crate::domain::entities::network_access_payload::NetworkAccessPayloadV1;
use crate::domain::error::GenesisError;
use crate::domain::ports::{BoxFuture, EventHandler, IdentityInstancePort};

pub struct NetworkAccessEventHandler {
    identity_instances: Arc<dyn IdentityInstancePort>,
}

impl NetworkAccessEventHandler {
    pub fn new(identity_instances: Arc<dyn IdentityInstancePort>) -> Self {
        Self { identity_instances }
    }
}

impl EventHandler for NetworkAccessEventHandler {
    fn routing_key(&self) -> &str {
        "deployment.network_access"
    }

    fn handle<'a>(&'a self, event: ActionEvent) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async move {
            let payload = NetworkAccessPayloadV1::from_value(&event.payload)?;
            let reference = IdentityInstanceRef::for_deployment(
                payload.deployment_id,
                payload.namespace.clone(),
            );

            info!(
                deployment_id = %payload.deployment_id,
                ranges = payload.ranges().len(),
                "applying the allow list"
            );

            self.identity_instances
                .set_allowed_cidrs(&reference, payload.allowed_cidrs)
                .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::identity_instance::DesiredIdentityInstance;
    use chrono::Utc;
    use serde_json::json;
    use std::sync::Mutex;
    use uuid::Uuid;

    const DEPLOYMENT: Uuid = Uuid::from_u128(1);

    #[derive(Default)]
    struct SpyInstances {
        writes: Mutex<Vec<(IdentityInstanceRef, Option<Vec<String>>)>>,
    }

    impl SpyInstances {
        fn writes(&self) -> Vec<(IdentityInstanceRef, Option<Vec<String>>)> {
            self.writes.lock().expect("not poisoned").clone()
        }
    }

    impl IdentityInstancePort for SpyInstances {
        fn apply<'a>(
            &'a self,
            _desired: &'a DesiredIdentityInstance,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            Box::pin(async { Ok(()) })
        }

        fn delete<'a>(
            &'a self,
            _reference: &'a IdentityInstanceRef,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            Box::pin(async { Ok(()) })
        }

        fn set_allowed_cidrs<'a>(
            &'a self,
            reference: &'a IdentityInstanceRef,
            ranges: Option<Vec<String>>,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            self.writes
                .lock()
                .expect("not poisoned")
                .push((reference.clone(), ranges));
            Box::pin(async { Ok(()) })
        }
    }

    fn event(allowed: serde_json::Value) -> ActionEvent {
        ActionEvent {
            action_id: Uuid::new_v4(),
            deployment_id: DEPLOYMENT,
            dataplane_id: Uuid::new_v4(),
            routing_key: "deployment.network_access".to_string(),
            version: 1,
            payload: json!({
                "deployment_id": DEPLOYMENT,
                "dataplane_id": Uuid::nil(),
                "organisation_id": Uuid::nil(),
                "namespace": "tenant-a",
                "kind": "ferriskey",
                "allowed_cidrs": allowed,
            }),
            occurred_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn the_ranges_reach_the_instance_in_its_own_namespace() {
        let instances = Arc::new(SpyInstances::default());
        let handler = NetworkAccessEventHandler::new(instances.clone());

        handler
            .handle(event(json!(["203.0.113.0/24"])))
            .await
            .expect("applied");

        let writes = instances.writes();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].0.namespace, "tenant-a");
        assert_eq!(writes[0].0.name, format!("deployment-{DEPLOYMENT}"));
        assert_eq!(writes[0].1, Some(vec!["203.0.113.0/24".to_string()]));
    }

    /// Open travels as null. Written through as null rather than as an empty
    /// list so the instance goes back to carrying no rule at all, which is
    /// what removes the policy on the other side.
    #[tokio::test]
    async fn going_back_to_open_clears_the_field() {
        let instances = Arc::new(SpyInstances::default());
        let handler = NetworkAccessEventHandler::new(instances.clone());

        handler.handle(event(json!(null))).await.expect("applied");

        assert_eq!(instances.writes()[0].1, None);
    }

    /// The bus delivers at least once. The action carries a state rather than
    /// a change, so the second delivery writes the same thing -- which is the
    /// whole reason it carries a state.
    #[tokio::test]
    async fn a_redelivery_writes_the_same_rule() {
        let instances = Arc::new(SpyInstances::default());
        let handler = NetworkAccessEventHandler::new(instances.clone());

        handler
            .handle(event(json!(["10.0.0.0/8"])))
            .await
            .expect("applied");
        handler
            .handle(event(json!(["10.0.0.0/8"])))
            .await
            .expect("applied again");

        let writes = instances.writes();
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0].1, writes[1].1);
    }

    #[tokio::test]
    async fn a_payload_that_is_not_a_network_rule_is_refused() {
        let instances = Arc::new(SpyInstances::default());
        let handler = NetworkAccessEventHandler::new(instances.clone());

        let mut event = event(json!(null));
        event.payload = json!({ "deployment_id": DEPLOYMENT });

        let error = handler.handle(event).await.expect_err("not a network rule");

        assert!(matches!(error, GenesisError::InvalidPayload { .. }));
        assert!(instances.writes().is_empty());
    }

    #[test]
    fn answers_to_the_network_access_routing_key() {
        let handler = NetworkAccessEventHandler::new(Arc::new(SpyInstances::default()));

        assert_eq!(handler.routing_key(), "deployment.network_access");
    }
}
