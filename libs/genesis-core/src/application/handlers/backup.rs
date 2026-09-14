//! Turning a `deployment.backup` action into one archive of an instance.
//!
//! The whole mechanism behind it already exists: the operator reconciles an
//! `IdentityInstanceBackup` into a CloudNativePG `Backup`, and Herald reports
//! every one it observes. This is the link that was missing -- the control
//! plane recording that somebody asked, and the data plane creating the
//! resource that answers.

use std::sync::Arc;

use tracing::info;

use crate::domain::entities::action_event::ActionEvent;
use crate::domain::entities::deployment_payload::DeploymentPayloadV1;
use crate::domain::entities::identity_instance::IdentityInstanceRef;
use crate::domain::error::GenesisError;
use crate::domain::ports::{BoxFuture, EventHandler, IdentityInstancePort};

pub struct BackupEventHandler {
    identity_instances: Arc<dyn IdentityInstancePort>,
}

impl BackupEventHandler {
    pub fn new(identity_instances: Arc<dyn IdentityInstancePort>) -> Self {
        Self { identity_instances }
    }

    /// What the archive resource is called.
    ///
    /// Taken from the action id rather than generated. Delivery is
    /// at-least-once by design, so a name invented per delivery would read the
    /// whole database twice for one request -- and leave two archives where
    /// somebody asked for one.
    fn archive_name(event: &ActionEvent) -> String {
        format!("ask-{}", event.action_id)
    }
}

impl EventHandler for BackupEventHandler {
    fn routing_key(&self) -> &str {
        "deployment.backup"
    }

    fn handle<'a>(&'a self, event: ActionEvent) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async move {
            let name = Self::archive_name(&event);
            let payload = DeploymentPayloadV1::from_value(&event.payload)?;
            let reference = IdentityInstanceRef::for_deployment(
                payload.deployment_id,
                payload.namespace.clone(),
            );

            info!(
                deployment_id = %payload.deployment_id,
                archive = %name,
                "an archive was asked for"
            );

            self.identity_instances
                .take_archive(&reference, &name)
                .await
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    use super::*;
    use crate::domain::entities::identity_instance::DesiredIdentityInstance;

    const DEPLOYMENT: Uuid = Uuid::from_u128(1);
    const ACTION: Uuid = Uuid::from_u128(7);

    #[derive(Default)]
    struct SpyInstances {
        asked: Mutex<Vec<(String, String)>>,
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
            _reference: &'a IdentityInstanceRef,
            _ranges: Option<Vec<String>>,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            Box::pin(async { Ok(()) })
        }

        fn take_archive<'a>(
            &'a self,
            reference: &'a IdentityInstanceRef,
            name: &'a str,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            Box::pin(async move {
                self.asked
                    .lock()
                    .expect("not poisoned")
                    .push((reference.namespace.clone(), name.to_string()));

                Ok(())
            })
        }
    }

    fn an_ask() -> ActionEvent {
        ActionEvent {
            action_id: ACTION,
            deployment_id: DEPLOYMENT,
            dataplane_id: Uuid::from_u128(2),
            routing_key: "deployment.backup".to_string(),
            version: 1,
            payload: json!({
                "deployment_id": DEPLOYMENT,
                "dataplane_id": Uuid::from_u128(2),
                "organisation_id": Uuid::from_u128(3),
                "name": "auth",
                "kind": "ferriskey",
                "version": "0.5.0",
                "namespace": "production-auth-0000000a",
                "created_by": Uuid::from_u128(4),
                "cpu_millis": 500,
                "memory_mib": 1024,
                "storage_gib": 5,
            }),
            occurred_at: Utc::now(),
        }
    }

    #[test]
    fn answers_to_the_backup_routing_key() {
        let handler = BackupEventHandler::new(Arc::new(SpyInstances::default()));

        assert_eq!(handler.routing_key(), "deployment.backup");
    }

    #[tokio::test]
    async fn an_ask_becomes_one_archive_beside_its_instance() {
        let instances = Arc::new(SpyInstances::default());
        let handler = BackupEventHandler::new(instances.clone());

        handler.handle(an_ask()).await.expect("the archive");

        assert_eq!(
            instances.asked.lock().expect("not poisoned").clone(),
            vec![(
                "production-auth-0000000a".to_string(),
                format!("ask-{ACTION}")
            )]
        );
    }

    /// Delivery is at-least-once by design. A name generated per delivery
    /// would read the whole database twice for one request, and leave two
    /// archives where somebody asked for one.
    #[tokio::test]
    async fn the_same_ask_delivered_twice_names_one_archive() {
        let instances = Arc::new(SpyInstances::default());
        let handler = BackupEventHandler::new(instances.clone());

        handler.handle(an_ask()).await.expect("the first");
        handler.handle(an_ask()).await.expect("the redelivery");

        let asked = instances.asked.lock().expect("not poisoned").clone();
        assert_eq!(asked.len(), 2, "both deliveries were carried out");
        assert_eq!(
            asked[0], asked[1],
            "and both named the same archive, so the second re-asserts the first"
        );
    }
}
