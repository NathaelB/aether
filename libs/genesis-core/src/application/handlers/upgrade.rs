use std::sync::Arc;

use tracing::info;

use crate::domain::entities::action_event::ActionEvent;
use crate::domain::entities::identity_instance::DesiredUpgrade;
use crate::domain::entities::upgrade_payload::UpgradePayloadV1;
use crate::domain::error::GenesisError;
use crate::domain::ports::{BoxFuture, EventHandler, IdentityInstanceUpgradePort};

/// Turns a `deployment.upgrade` action into the custom resource the operator
/// already knows how to reconcile.
///
/// The operator has driven `IdentityInstanceUpgrade` since it was written:
/// approval gate, rolling strategy, version patch, readiness wait, cleanup.
/// Nothing created one. This is what does.
pub struct UpgradeEventHandler {
    upgrades: Arc<dyn IdentityInstanceUpgradePort>,
}

impl UpgradeEventHandler {
    pub fn new(upgrades: Arc<dyn IdentityInstanceUpgradePort>) -> Self {
        Self { upgrades }
    }
}

impl EventHandler for UpgradeEventHandler {
    fn routing_key(&self) -> &str {
        "deployment.upgrade"
    }

    fn handle<'a>(&'a self, event: ActionEvent) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async move {
            let payload = UpgradePayloadV1::from_value(&event.payload)?;

            let desired = DesiredUpgrade::for_deployment(
                payload.deployment_id,
                payload.namespace.clone(),
                payload.to_version.clone(),
            );

            // The bus delivers at least once, so the same event arrives again
            // after a restart, a redelivery or a lease that lapsed. A second
            // resource for the same step would race the first.
            match self.upgrades.find(&desired.reference()).await? {
                Some(in_flight) if in_flight.target_version == desired.target_version => {
                    info!(
                        deployment_id = %payload.deployment_id,
                        target = %desired.target_version,
                        "the upgrade is already there; nothing to do"
                    );
                    Ok(())
                }
                Some(in_flight) => Err(GenesisError::UpgradeAlreadyInFlight {
                    deployment_id: payload.deployment_id,
                    in_flight: in_flight.target_version,
                    requested: desired.target_version,
                }),
                None => {
                    info!(
                        deployment_id = %payload.deployment_id,
                        from = %payload.from_version,
                        to = %desired.target_version,
                        "creating the upgrade"
                    );
                    self.upgrades.create(&desired).await
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::identity_instance::{InFlightUpgrade, UpgradeRef};
    use chrono::Utc;
    use serde_json::json;
    use std::sync::Mutex;
    use uuid::Uuid;

    const DEPLOYMENT: Uuid = Uuid::from_u128(1);

    #[derive(Default)]
    struct SpyUpgrades {
        existing: Mutex<Option<InFlightUpgrade>>,
        created: Mutex<Vec<DesiredUpgrade>>,
    }

    impl SpyUpgrades {
        fn holding(target: &str) -> Self {
            Self {
                existing: Mutex::new(Some(InFlightUpgrade {
                    target_version: target.to_string(),
                })),
                created: Mutex::new(Vec::new()),
            }
        }

        fn created(&self) -> Vec<DesiredUpgrade> {
            self.created.lock().expect("not poisoned").clone()
        }
    }

    impl IdentityInstanceUpgradePort for SpyUpgrades {
        fn find<'a>(
            &'a self,
            _reference: &'a UpgradeRef,
        ) -> BoxFuture<'a, Result<Option<InFlightUpgrade>, GenesisError>> {
            let found = self.existing.lock().expect("not poisoned").clone();
            Box::pin(async move { Ok(found) })
        }

        fn create<'a>(
            &'a self,
            desired: &'a DesiredUpgrade,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            self.created
                .lock()
                .expect("not poisoned")
                .push(desired.clone());
            Box::pin(async { Ok(()) })
        }
    }

    fn event(to: &str) -> ActionEvent {
        ActionEvent {
            action_id: Uuid::new_v4(),
            deployment_id: DEPLOYMENT,
            dataplane_id: Uuid::new_v4(),
            routing_key: "deployment.upgrade".to_string(),
            version: 1,
            payload: json!({
                "deployment_id": DEPLOYMENT,
                "namespace": "tenant-a",
                "from_version": "26.0.0",
                "to_version": to,
            }),
            occurred_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn creates_the_upgrade_in_the_deployment_namespace() {
        let upgrades = Arc::new(SpyUpgrades::default());
        let handler = UpgradeEventHandler::new(upgrades.clone());

        handler.handle(event("26.0.1")).await.expect("created");

        let created = upgrades.created();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].namespace, "tenant-a");
        assert_eq!(created[0].target_version, "26.0.1");
        // The operator matches upgrades on the instance name, not on the
        // upgrade's own name, so this is the field that has to be right.
        assert_eq!(created[0].instance_name, format!("deployment-{DEPLOYMENT}"));
    }

    /// The bus delivers at least once. A redelivery must not produce a second
    /// resource racing the first.
    #[tokio::test]
    async fn a_redelivery_of_the_same_step_creates_nothing() {
        let upgrades = Arc::new(SpyUpgrades::holding("26.0.1"));
        let handler = UpgradeEventHandler::new(upgrades.clone());

        handler.handle(event("26.0.1")).await.expect("a no-op");

        assert!(
            upgrades.created().is_empty(),
            "a second resource was created for the same step"
        );
    }

    /// A different target while one is in flight is refused, not applied. The
    /// one running may already have patched the instance, and pointing it
    /// somewhere else mid-step is how a version nobody asked for ends up
    /// running.
    #[tokio::test]
    async fn a_different_target_while_one_is_in_flight_is_refused() {
        let upgrades = Arc::new(SpyUpgrades::holding("26.0.1"));
        let handler = UpgradeEventHandler::new(upgrades.clone());

        let error = handler
            .handle(event("27.0.0"))
            .await
            .expect_err("one is already in flight");

        assert!(matches!(error, GenesisError::UpgradeAlreadyInFlight { .. }));
        assert!(upgrades.created().is_empty());
    }

    /// The refusal names both versions, because the person reading it needs to
    /// know which one is actually being applied.
    #[tokio::test]
    async fn the_refusal_names_both_versions() {
        let handler = UpgradeEventHandler::new(Arc::new(SpyUpgrades::holding("26.0.1")));

        let message = handler
            .handle(event("27.0.0"))
            .await
            .expect_err("in flight")
            .to_string();

        assert!(message.contains("26.0.1"), "{message}");
        assert!(message.contains("27.0.0"), "{message}");
    }

    /// A create payload reaching this handler is a routing mistake. Applying
    /// half of it would be worse than refusing.
    #[tokio::test]
    async fn a_payload_that_is_not_an_upgrade_is_refused() {
        let upgrades = Arc::new(SpyUpgrades::default());
        let handler = UpgradeEventHandler::new(upgrades.clone());

        let mut event = event("26.0.1");
        event.payload = json!({ "deployment_id": DEPLOYMENT, "namespace": "tenant-a" });

        let error = handler.handle(event).await.expect_err("not an upgrade");

        assert!(matches!(error, GenesisError::InvalidPayload { .. }));
        assert!(upgrades.created().is_empty());
    }

    #[test]
    fn answers_to_the_upgrade_routing_key() {
        let handler = UpgradeEventHandler::new(Arc::new(SpyUpgrades::default()));

        assert_eq!(handler.routing_key(), "deployment.upgrade");
    }
}
