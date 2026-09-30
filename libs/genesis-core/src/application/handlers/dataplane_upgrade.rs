use std::sync::Arc;

use tracing::{info, warn};

use crate::domain::entities::action_event::ActionEvent;
use crate::domain::entities::dataplane_upgrade::DesiredDataplaneUpgrade;
use crate::domain::entities::dataplane_upgrade_payload::DataplaneUpgradePayloadV1;
use crate::domain::error::GenesisError;
use crate::domain::ports::{BoxFuture, DataplaneUpgradePort, EventHandler};

pub struct DataplaneUpgradeEventHandler {
    upgrades: Arc<dyn DataplaneUpgradePort>,
    namespace: String,
}

impl DataplaneUpgradeEventHandler {
    pub fn new(upgrades: Arc<dyn DataplaneUpgradePort>, namespace: impl Into<String>) -> Self {
        Self {
            upgrades,
            namespace: namespace.into(),
        }
    }
}

impl EventHandler for DataplaneUpgradeEventHandler {
    fn routing_key(&self) -> &str {
        "dataplane.upgrade"
    }

    fn handle<'a>(&'a self, event: ActionEvent) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async move {
            let payload = DataplaneUpgradePayloadV1::from_value(&event.payload)?;
            let desired = DesiredDataplaneUpgrade::from_payload(&payload, &self.namespace);

            match self.upgrades.find(&desired.reference()).await? {
                Some(existing)
                    if existing.target_version == desired.target_version
                        && (!existing.finished || existing.succeeded) =>
                {
                    info!(
                        dataplane_id = %desired.dataplane_id,
                        target = %desired.target_version,
                        "the data plane upgrade is already there; nothing to do"
                    );
                    Ok(())
                }
                Some(existing) if existing.finished => {
                    info!(
                        dataplane_id = %desired.dataplane_id,
                        previous = %existing.target_version,
                        target = %desired.target_version,
                        "the previous data plane upgrade is finished; replacing it"
                    );
                    self.upgrades.replace(&desired).await
                }
                Some(existing) => {
                    warn!(
                        dataplane_id = %desired.dataplane_id,
                        in_flight = %existing.target_version,
                        requested = %desired.target_version,
                        "refusing a different target while a data plane upgrade is in flight"
                    );
                    Err(GenesisError::Handler {
                        message: format!(
                            "data plane {} is already upgrading to {}, not {}",
                            desired.dataplane_id, existing.target_version, desired.target_version
                        ),
                    })
                }
                None => {
                    info!(
                        dataplane_id = %desired.dataplane_id,
                        target = %desired.target_version,
                        "creating the data plane upgrade"
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
    use crate::domain::entities::dataplane_upgrade::{
        DataplaneUpgradeRef, ExistingDataplaneUpgrade,
    };
    use crate::domain::entities::dataplane_upgrade_payload::{UpgradeComponent, UpgradeStrategy};
    use chrono::Utc;
    use serde_json::json;
    use std::sync::Mutex;
    use uuid::Uuid;

    const DATAPLANE: Uuid = Uuid::from_u128(2);

    #[derive(Default)]
    struct SpyUpgrades {
        existing: Mutex<Option<ExistingDataplaneUpgrade>>,
        created: Mutex<Vec<DesiredDataplaneUpgrade>>,
        replaced: Mutex<Vec<DesiredDataplaneUpgrade>>,
    }

    impl SpyUpgrades {
        fn holding(target: &str, finished: bool) -> Self {
            Self {
                existing: Mutex::new(Some(ExistingDataplaneUpgrade {
                    target_version: target.to_string(),
                    finished,
                    succeeded: finished,
                })),
                ..Default::default()
            }
        }

        fn holding_unsuccessful(target: &str) -> Self {
            Self {
                existing: Mutex::new(Some(ExistingDataplaneUpgrade {
                    target_version: target.to_string(),
                    finished: true,
                    succeeded: false,
                })),
                ..Default::default()
            }
        }

        fn created(&self) -> Vec<DesiredDataplaneUpgrade> {
            self.created.lock().expect("not poisoned").clone()
        }

        fn replaced(&self) -> Vec<DesiredDataplaneUpgrade> {
            self.replaced.lock().expect("not poisoned").clone()
        }
    }

    impl DataplaneUpgradePort for SpyUpgrades {
        fn find<'a>(
            &'a self,
            _reference: &'a DataplaneUpgradeRef,
        ) -> BoxFuture<'a, Result<Option<ExistingDataplaneUpgrade>, GenesisError>> {
            let found = self.existing.lock().expect("not poisoned").clone();
            Box::pin(async move { Ok(found) })
        }

        fn create<'a>(
            &'a self,
            desired: &'a DesiredDataplaneUpgrade,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            self.created
                .lock()
                .expect("not poisoned")
                .push(desired.clone());
            *self.existing.lock().expect("not poisoned") = Some(ExistingDataplaneUpgrade {
                target_version: desired.target_version.clone(),
                finished: false,
                succeeded: false,
            });
            Box::pin(async { Ok(()) })
        }

        fn replace<'a>(
            &'a self,
            desired: &'a DesiredDataplaneUpgrade,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            self.replaced
                .lock()
                .expect("not poisoned")
                .push(desired.clone());
            Box::pin(async { Ok(()) })
        }
    }

    fn handler(upgrades: &Arc<SpyUpgrades>) -> DataplaneUpgradeEventHandler {
        DataplaneUpgradeEventHandler::new(upgrades.clone(), "aether-dataplane")
    }

    fn event_with(payload: serde_json::Value) -> ActionEvent {
        ActionEvent {
            action_id: Uuid::new_v4(),
            deployment_id: Some(Uuid::new_v4()),
            dataplane_id: DATAPLANE,
            routing_key: "dataplane.upgrade".to_string(),
            version: 1,
            payload,
            occurred_at: Utc::now(),
        }
    }

    fn event(to: &str) -> ActionEvent {
        event_with(json!({
            "dataplane_id": DATAPLANE,
            "target_version": to,
            "components": ["Herald", "Operator"],
            "strategy": "canary",
            "max_unavailable": 2,
        }))
    }

    #[tokio::test]
    async fn creates_one_upgrade_named_after_the_data_plane_with_the_mapped_spec() {
        let upgrades = Arc::new(SpyUpgrades::default());

        handler(&upgrades)
            .handle(event("26.1.0"))
            .await
            .expect("created");

        let created = upgrades.created();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].name, DATAPLANE.to_string());
        assert_eq!(created[0].namespace, "aether-dataplane");
        assert_eq!(created[0].dataplane_id, DATAPLANE.to_string());
        assert_eq!(created[0].target_version, "26.1.0");
        assert_eq!(
            created[0].components,
            vec![UpgradeComponent::Herald, UpgradeComponent::Operator]
        );
        assert_eq!(created[0].strategy, UpgradeStrategy::Canary);
        assert_eq!(created[0].max_unavailable, 2);
    }

    #[tokio::test]
    async fn the_same_event_twice_creates_one_upgrade() {
        let upgrades = Arc::new(SpyUpgrades::default());
        let handler = handler(&upgrades);

        handler.handle(event("26.1.0")).await.expect("created");
        handler.handle(event("26.1.0")).await.expect("a no-op");

        assert_eq!(upgrades.created().len(), 1);
        assert!(upgrades.replaced().is_empty());
    }

    #[tokio::test]
    async fn a_finished_upgrade_for_the_same_version_is_left_alone() {
        let upgrades = Arc::new(SpyUpgrades::holding("26.1.0", true));

        handler(&upgrades)
            .handle(event("26.1.0"))
            .await
            .expect("a no-op");

        assert!(upgrades.created().is_empty());
        assert!(upgrades.replaced().is_empty());
    }

    #[tokio::test]
    async fn a_failed_or_rolled_back_upgrade_can_be_retried_at_the_same_version() {
        let upgrades = Arc::new(SpyUpgrades::holding_unsuccessful("26.1.0"));

        handler(&upgrades)
            .handle(event("26.1.0"))
            .await
            .expect("replaced");

        let replaced = upgrades.replaced();
        assert_eq!(replaced.len(), 1);
        assert_eq!(replaced[0].target_version, "26.1.0");
        assert!(upgrades.created().is_empty());
    }

    #[tokio::test]
    async fn a_different_version_while_one_is_in_flight_is_refused() {
        let upgrades = Arc::new(SpyUpgrades::holding("26.1.0", false));

        let message = handler(&upgrades)
            .handle(event("27.0.0"))
            .await
            .expect_err("one is in flight")
            .to_string();

        assert!(message.contains("26.1.0"), "{message}");
        assert!(message.contains("27.0.0"), "{message}");
        assert!(upgrades.created().is_empty());
        assert!(upgrades.replaced().is_empty());
    }

    #[tokio::test]
    async fn a_finished_upgrade_makes_room_for_a_new_target() {
        let upgrades = Arc::new(SpyUpgrades::holding("26.1.0", true));

        handler(&upgrades)
            .handle(event("27.0.0"))
            .await
            .expect("replaced");

        let replaced = upgrades.replaced();
        assert_eq!(replaced.len(), 1);
        assert_eq!(replaced[0].target_version, "27.0.0");
        assert!(upgrades.created().is_empty());
    }

    #[tokio::test]
    async fn an_invalid_payload_never_reaches_the_port() {
        let upgrades = Arc::new(SpyUpgrades::default());
        let mut payload = event("26.1.0").payload;
        payload["components"] = json!([]);

        let error = handler(&upgrades)
            .handle(event_with(payload))
            .await
            .expect_err("no component");

        assert!(matches!(error, GenesisError::InvalidPayload { .. }));
        assert!(upgrades.created().is_empty());
        assert!(upgrades.replaced().is_empty());
    }

    #[test]
    fn answers_to_the_data_plane_upgrade_routing_key() {
        let handler = handler(&Arc::new(SpyUpgrades::default()));

        assert_eq!(handler.routing_key(), "dataplane.upgrade");
    }
}
