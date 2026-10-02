use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use aether_core::dataplane_upgrade::{
    DataplaneUpgradePayload, UpgradeComponent as ApiComponent, UpgradeStrategy as ApiStrategy,
};
use aether_domain::dataplane::value_objects::DataPlaneId as ApiDataPlaneId;
use chrono::Utc;
use genesis_core::application::handlers::dataplane_upgrade::DataplaneUpgradeEventHandler;
use genesis_core::domain::entities::action_event::ActionEvent as GenesisEvent;
use genesis_core::domain::entities::dataplane_upgrade::{
    DataplaneUpgradeRef, DesiredDataplaneUpgrade, ExistingDataplaneUpgrade,
};
use genesis_core::domain::error::GenesisError;
use genesis_core::domain::ports::{BoxFuture, DataplaneUpgradePort};
use herald_core::domain::entities::action::{Action, ActionEvent as HeraldEvent, ActionId};
use herald_core::domain::entities::dataplane::DataPlaneId as HeraldDataPlaneId;
use herald_core::domain::entities::deployment::DeploymentId;
use serde_json::Value;
use uuid::Uuid;

pub const NAMESPACE: &str = "aether-dataplane";
pub const PREVIOUS: &str = "26.0.0";
pub const TARGET: &str = "26.1.0";

#[derive(Debug, Clone)]
struct Stored {
    desired: DesiredDataplaneUpgrade,
    finished: bool,
    succeeded: bool,
}

#[derive(Debug, Default)]
pub struct InMemoryUpgrades {
    stored: Mutex<HashMap<String, Stored>>,
    creations: Mutex<usize>,
    replacements: Mutex<usize>,
}

impl InMemoryUpgrades {
    pub fn creations(&self) -> usize {
        *self.creations.lock().expect("not poisoned")
    }

    pub fn replacements(&self) -> usize {
        *self.replacements.lock().expect("not poisoned")
    }

    pub fn len(&self) -> usize {
        self.stored.lock().expect("not poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn desired(&self, name: &str) -> DesiredDataplaneUpgrade {
        self.stored.lock().expect("not poisoned")[name]
            .desired
            .clone()
    }

    pub fn finish(&self, name: &str, succeeded: bool) {
        let mut stored = self.stored.lock().expect("not poisoned");
        let entry = stored.get_mut(name).expect("an upgrade to finish");
        entry.finished = true;
        entry.succeeded = succeeded;
    }
}

impl DataplaneUpgradePort for InMemoryUpgrades {
    fn find<'a>(
        &'a self,
        reference: &'a DataplaneUpgradeRef,
    ) -> BoxFuture<'a, Result<Option<ExistingDataplaneUpgrade>, GenesisError>> {
        let found = self
            .stored
            .lock()
            .expect("not poisoned")
            .get(&reference.name)
            .map(|stored| ExistingDataplaneUpgrade {
                target_version: stored.desired.target_version.clone(),
                finished: stored.finished,
                succeeded: stored.succeeded,
            });
        Box::pin(async move { Ok(found) })
    }

    fn create<'a>(
        &'a self,
        desired: &'a DesiredDataplaneUpgrade,
    ) -> BoxFuture<'a, Result<(), GenesisError>> {
        *self.creations.lock().expect("not poisoned") += 1;
        self.store(desired);
        Box::pin(async { Ok(()) })
    }

    fn replace<'a>(
        &'a self,
        desired: &'a DesiredDataplaneUpgrade,
    ) -> BoxFuture<'a, Result<(), GenesisError>> {
        *self.replacements.lock().expect("not poisoned") += 1;
        self.store(desired);
        Box::pin(async { Ok(()) })
    }
}

impl InMemoryUpgrades {
    fn store(&self, desired: &DesiredDataplaneUpgrade) {
        self.stored.lock().expect("not poisoned").insert(
            desired.name.clone(),
            Stored {
                desired: desired.clone(),
                finished: false,
                succeeded: false,
            },
        );
    }
}

pub fn dataplane() -> Uuid {
    Uuid::from_u128(7)
}

pub fn api_payload(target: &str, components: Vec<ApiComponent>, max_unavailable: u32) -> Value {
    api_payload_with(target, components, ApiStrategy::Rolling, max_unavailable)
}

pub fn api_payload_with(
    target: &str,
    components: Vec<ApiComponent>,
    strategy: ApiStrategy,
    max_unavailable: u32,
) -> Value {
    DataplaneUpgradePayload::new(
        ApiDataPlaneId(dataplane()),
        target.to_string(),
        components,
        strategy,
        max_unavailable,
    )
    .expect("a valid payload")
    .into_action_payload()
    .data
}

pub fn herald_action(deployment: Option<Uuid>, action_type: &str, payload: Value) -> Action {
    Action {
        id: ActionId(Uuid::from_u128(1)),
        deployment_id: deployment.map(|id| DeploymentId::new(id.to_string())),
        dataplane_id: HeraldDataPlaneId::new(dataplane().to_string()),
        action_type: action_type.to_string(),
        payload,
        version: 1,
        occurred_at: Utc::now(),
    }
}

pub fn wire(action: Action) -> Value {
    let event = HeraldEvent::try_from(action).expect("a valid action");
    serde_json::to_value(&event).expect("serialisable")
}

pub fn upgrade_wire(target: &str) -> Value {
    upgrade_wire_with(target, ApiStrategy::Rolling)
}

pub fn upgrade_wire_with(target: &str, strategy: ApiStrategy) -> Value {
    let payload = api_payload_with(
        target,
        vec![
            ApiComponent::Herald,
            ApiComponent::Genesis,
            ApiComponent::Operator,
        ],
        strategy,
        1,
    );
    wire(herald_action(None, "dataplane.upgrade", payload))
}

pub fn genesis_event(wire: Value) -> GenesisEvent {
    serde_json::from_value(wire).expect("Genesis reads what Herald publishes")
}

pub fn handler(upgrades: &Arc<InMemoryUpgrades>) -> DataplaneUpgradeEventHandler {
    DataplaneUpgradeEventHandler::new(upgrades.clone(), NAMESPACE)
}
