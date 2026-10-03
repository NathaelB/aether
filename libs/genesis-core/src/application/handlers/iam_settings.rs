//! Turning a `deployment.iam_settings` action into the field the operator
//! reconciles.
//!
//! Like the allow list, the action carries the whole value every time, so a
//! redelivery writes what the first delivery wrote.

use std::sync::Arc;

use tracing::info;

use crate::domain::entities::action_event::ActionEvent;
use crate::domain::entities::iam_settings_payload::IamSettingsPayloadV1;
use crate::domain::entities::identity_instance::IdentityInstanceRef;
use crate::domain::error::GenesisError;
use crate::domain::ports::{BoxFuture, EventHandler, IdentityInstancePort};

pub struct IamSettingsEventHandler {
    identity_instances: Arc<dyn IdentityInstancePort>,
}

impl IamSettingsEventHandler {
    pub fn new(identity_instances: Arc<dyn IdentityInstancePort>) -> Self {
        Self { identity_instances }
    }
}

impl EventHandler for IamSettingsEventHandler {
    fn routing_key(&self) -> &str {
        "deployment.iam_settings"
    }

    fn handle<'a>(&'a self, event: ActionEvent) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async move {
            let payload = IamSettingsPayloadV1::from_value(&event.payload)?;
            let reference =
                IdentityInstanceRef::for_deployment(payload.deployment_id, payload.namespace);

            info!(
                deployment_id = %payload.deployment_id,
                branding = payload.branding.is_some(),
                "applying the IAM settings"
            );

            self.identity_instances
                .set_iam(&reference, payload.branding)
                .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::iam_settings_payload::Branding;
    use crate::domain::entities::identity_instance::DesiredIdentityInstance;
    use chrono::Utc;
    use serde_json::json;
    use std::sync::Mutex;
    use uuid::Uuid;

    const DEPLOYMENT: Uuid = Uuid::from_u128(1);

    #[derive(Default)]
    struct SpyInstances {
        writes: Mutex<Vec<(IdentityInstanceRef, Option<Branding>)>>,
    }

    impl SpyInstances {
        fn writes(&self) -> Vec<(IdentityInstanceRef, Option<Branding>)> {
            self.writes.lock().expect("not poisoned").clone()
        }
    }

    impl IdentityInstancePort for SpyInstances {
        fn take_archive<'a>(
            &'a self,
            _reference: &'a IdentityInstanceRef,
            _name: &'a str,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            Box::pin(async { Ok(()) })
        }

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
            _reference: &'a IdentityInstanceRef,
            _ranges: Option<Vec<String>>,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            Box::pin(async { Ok(()) })
        }

        fn set_iam<'a>(
            &'a self,
            reference: &'a IdentityInstanceRef,
            branding: Option<Branding>,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            self.writes
                .lock()
                .expect("not poisoned")
                .push((reference.clone(), branding));
            Box::pin(async { Ok(()) })
        }

        fn is_ready<'a>(
            &'a self,
            _reference: &'a IdentityInstanceRef,
        ) -> BoxFuture<'a, Result<bool, GenesisError>> {
            Box::pin(async { Ok(true) })
        }

        fn database_uri<'a>(
            &'a self,
            _reference: &'a IdentityInstanceRef,
        ) -> BoxFuture<'a, Result<String, GenesisError>> {
            Box::pin(async { Ok(String::new()) })
        }

        fn delete_namespace<'a>(
            &'a self,
            _namespace: &'a str,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            Box::pin(async { Ok(()) })
        }
    }

    fn event(branding: serde_json::Value) -> ActionEvent {
        ActionEvent {
            action_id: Uuid::new_v4(),
            deployment_id: Some(DEPLOYMENT),
            dataplane_id: Uuid::new_v4(),
            routing_key: "deployment.iam_settings".to_string(),
            version: 1,
            payload: json!({
                "deployment_id": DEPLOYMENT,
                "namespace": "tenant-a",
                "branding": branding,
            }),
            occurred_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn the_branding_reaches_the_instance_in_its_own_namespace() {
        let instances = Arc::new(SpyInstances::default());
        let handler = IamSettingsEventHandler::new(instances.clone());

        handler
            .handle(event(json!({ "radius": 8 })))
            .await
            .expect("applied");

        let writes = instances.writes();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].0.namespace, "tenant-a");
        assert_eq!(writes[0].0.name, format!("deployment-{DEPLOYMENT}"));
        assert_eq!(writes[0].1.as_ref().expect("branding").radius, Some(8));
    }

    #[tokio::test]
    async fn a_null_branding_clears_the_field() {
        let instances = Arc::new(SpyInstances::default());
        let handler = IamSettingsEventHandler::new(instances.clone());

        handler.handle(event(json!(null))).await.expect("applied");

        assert_eq!(instances.writes()[0].1, None);
    }

    #[tokio::test]
    async fn a_redelivery_writes_the_same_value() {
        let instances = Arc::new(SpyInstances::default());
        let handler = IamSettingsEventHandler::new(instances.clone());
        let branding = json!({ "colors": { "primary": "#112233" }, "radius": 2 });

        handler
            .handle(event(branding.clone()))
            .await
            .expect("applied");
        handler
            .handle(event(branding))
            .await
            .expect("applied again");

        let writes = instances.writes();
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0], writes[1]);
    }

    #[tokio::test]
    async fn an_invalid_payload_never_reaches_the_port() {
        let instances = Arc::new(SpyInstances::default());
        let handler = IamSettingsEventHandler::new(instances.clone());

        for branding in [
            json!({ "radius": 25 }),
            json!({ "colors": { "primary": "blue" } }),
            json!({ "unknown": 1 }),
        ] {
            let error = handler.handle(event(branding)).await.expect_err("invalid");
            assert!(matches!(error, GenesisError::InvalidPayload { .. }));
        }

        let mut malformed = event(json!(null));
        malformed.payload = json!({ "namespace": "tenant-a" });
        assert!(handler.handle(malformed).await.is_err());
        assert!(instances.writes().is_empty());
    }

    #[test]
    fn answers_to_the_iam_settings_routing_key() {
        let handler = IamSettingsEventHandler::new(Arc::new(SpyInstances::default()));

        assert_eq!(handler.routing_key(), "deployment.iam_settings");
    }
}
