use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use aether_core::dataplane_upgrade::{
    DataplaneUpgradePayload, UpgradeComponent as ApiComponent, UpgradeStrategy as ApiStrategy,
};
use aether_crds::v1alpha::identity_dataplane_upgrade::{
    DataplaneUpgradePhase, IdentityDataplaneUpgrade, IdentityDataplaneUpgradeStatus,
};
use aether_domain::dataplane::value_objects::DataPlaneId as ApiDataPlaneId;
use aether_operator_core::domain::dataplane_upgrade::service::{
    Observed, Step, advance, decide, preflight,
};
use aether_operator_core::domain::dataplane_upgrade::{ComponentVersions, DataplaneComponentKind};
use chrono::{DateTime, Duration as Delta, Utc};
use genesis_core::application::handlers::dataplane_upgrade::DataplaneUpgradeEventHandler;
use genesis_core::domain::entities::action_event::ActionEvent as GenesisEvent;
use genesis_core::domain::entities::dataplane_upgrade::{
    DataplaneUpgradeRef, DesiredDataplaneUpgrade, ExistingDataplaneUpgrade,
};
use genesis_core::domain::entities::dataplane_upgrade_payload::DataplaneUpgradePayloadV1;
use genesis_core::domain::error::GenesisError;
use genesis_core::domain::ports::{BoxFuture, DataplaneUpgradePort, EventHandler};
use genesis_core::infrastructure::kubernetes::dataplane_upgrade::resource;
use herald_core::domain::entities::action::{Action, ActionEvent as HeraldEvent, ActionId};
use herald_core::domain::entities::dataplane::DataPlaneId as HeraldDataPlaneId;
use herald_core::domain::entities::deployment::DeploymentId;
use serde_json::{Value, json};
use uuid::Uuid;

const NAMESPACE: &str = "aether-dataplane";
const TIMEOUT: Duration = Duration::from_secs(300);
const TICK: i64 = 10;
const PREVIOUS: &str = "26.0.0";
const TARGET: &str = "26.1.0";

#[derive(Default)]
struct InMemoryUpgrades {
    stored: Mutex<HashMap<String, (DesiredDataplaneUpgrade, bool)>>,
    creations: Mutex<usize>,
}

impl InMemoryUpgrades {
    fn creations(&self) -> usize {
        *self.creations.lock().expect("not poisoned")
    }

    fn desired(&self, name: &str) -> DesiredDataplaneUpgrade {
        self.stored.lock().expect("not poisoned")[name].0.clone()
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
            .map(|(desired, finished)| ExistingDataplaneUpgrade {
                target_version: desired.target_version.clone(),
                finished: *finished,
                succeeded: *finished,
            });
        Box::pin(async move { Ok(found) })
    }

    fn create<'a>(
        &'a self,
        desired: &'a DesiredDataplaneUpgrade,
    ) -> BoxFuture<'a, Result<(), GenesisError>> {
        *self.creations.lock().expect("not poisoned") += 1;
        self.stored
            .lock()
            .expect("not poisoned")
            .insert(desired.name.clone(), (desired.clone(), false));
        Box::pin(async { Ok(()) })
    }

    fn replace<'a>(
        &'a self,
        desired: &'a DesiredDataplaneUpgrade,
    ) -> BoxFuture<'a, Result<(), GenesisError>> {
        self.stored
            .lock()
            .expect("not poisoned")
            .insert(desired.name.clone(), (desired.clone(), false));
        Box::pin(async { Ok(()) })
    }
}

fn dataplane() -> Uuid {
    Uuid::from_u128(7)
}

fn api_payload(target: &str, components: Vec<ApiComponent>, max_unavailable: u32) -> Value {
    DataplaneUpgradePayload::new(
        ApiDataPlaneId(dataplane()),
        target.to_string(),
        components,
        ApiStrategy::Rolling,
        max_unavailable,
    )
    .expect("a valid payload")
    .into_action_payload()
    .data
}

fn herald_action(deployment: Option<Uuid>, action_type: &str, payload: Value) -> Action {
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

fn wire(action: Action) -> Value {
    let event = HeraldEvent::try_from(action).expect("a valid action");
    serde_json::to_value(&event).expect("serialisable")
}

fn upgrade_wire(target: &str) -> Value {
    let payload = api_payload(
        target,
        vec![
            ApiComponent::Herald,
            ApiComponent::Genesis,
            ApiComponent::Operator,
        ],
        1,
    );
    wire(herald_action(None, "dataplane.upgrade", payload))
}

fn genesis_event(wire: Value) -> GenesisEvent {
    serde_json::from_value(wire).expect("Genesis reads what Herald publishes")
}

fn handler(upgrades: &Arc<InMemoryUpgrades>) -> DataplaneUpgradeEventHandler {
    DataplaneUpgradeEventHandler::new(upgrades.clone(), NAMESPACE)
}

struct Cluster {
    versions: ComponentVersions,
    ready: BTreeMap<DataplaneComponentKind, bool>,
    never_ready: Option<DataplaneComponentKind>,
    patched: Vec<DataplaneComponentKind>,
    restored: Vec<(DataplaneComponentKind, String)>,
}

impl Cluster {
    fn at(version: &str) -> Self {
        Self {
            versions: DataplaneComponentKind::ALL
                .into_iter()
                .map(|kind| (kind, version.to_string()))
                .collect(),
            ready: DataplaneComponentKind::ALL
                .into_iter()
                .map(|kind| (kind, true))
                .collect(),
            never_ready: None,
            patched: Vec::new(),
            restored: Vec::new(),
        }
    }

    fn observed(&self, now: DateTime<Utc>) -> Observed {
        Observed {
            versions: self.versions.clone(),
            ready: self.ready.clone(),
            now,
        }
    }

    fn apply(&mut self, step: &Step, target: &str) {
        match step {
            Step::Patch(kind) => {
                self.patched.push(*kind);
                self.versions.insert(*kind, target.to_string());
                self.ready.insert(*kind, self.never_ready != Some(*kind));
            }
            Step::RollBack { restore, .. } => {
                for (kind, version) in restore {
                    self.versions.insert(*kind, version.clone());
                    self.ready.insert(*kind, true);
                    self.restored.push((*kind, version.clone()));
                }
            }
            _ => {}
        }
    }
}

fn drive(
    upgrade: &IdentityDataplaneUpgrade,
    cluster: &mut Cluster,
) -> IdentityDataplaneUpgradeStatus {
    let spec = &upgrade.spec;
    let mut status = upgrade.status.clone().unwrap_or_default();
    assert_eq!(status.phase, DataplaneUpgradePhase::Pending);
    let start = Utc::now();

    for tick in 0..200 {
        let now = start + Delta::seconds(tick * TICK);
        let step = preflight(spec, &status)
            .unwrap_or_else(|| decide(spec, &status, &cluster.observed(now), TIMEOUT));
        status = advance(spec, &status, &step, now);
        cluster.apply(&step, &spec.target_version);
        if matches!(step, Step::Idle) {
            return status;
        }
    }
    panic!("the upgrade never settled, last status: {status:?}");
}

async fn desired_after_the_handler(
    wire: Value,
) -> (Arc<InMemoryUpgrades>, DesiredDataplaneUpgrade) {
    let upgrades = Arc::new(InMemoryUpgrades::default());
    handler(&upgrades)
        .handle(genesis_event(wire))
        .await
        .expect("the handler accepts the event");
    let desired = upgrades.desired(&dataplane().to_string());
    (upgrades, desired)
}

#[tokio::test]
async fn an_upgrade_flows_from_the_api_payload_to_a_completed_status() {
    let (_, desired) = desired_after_the_handler(upgrade_wire(TARGET)).await;
    assert_eq!(desired.name, dataplane().to_string());
    assert_eq!(desired.namespace, NAMESPACE);

    let upgrade = resource(&desired);
    assert_eq!(upgrade.spec.target_version, TARGET);
    let mut cluster = Cluster::at(PREVIOUS);

    let status = drive(&upgrade, &mut cluster);

    assert_eq!(status.phase, DataplaneUpgradePhase::Completed);
    assert_eq!(status.current_version.as_deref(), Some(TARGET));
    assert_eq!(status.progress.as_deref(), Some("3/3 components updated"));
    assert_eq!(
        cluster.patched,
        vec![
            DataplaneComponentKind::Herald,
            DataplaneComponentKind::Genesis,
            DataplaneComponentKind::Operator,
        ]
    );
    assert!(cluster.versions.values().all(|version| version == TARGET));
}

#[tokio::test]
async fn a_component_that_never_becomes_ready_rolls_the_others_back_in_reverse_order() {
    let (_, desired) = desired_after_the_handler(upgrade_wire(TARGET)).await;
    let upgrade = resource(&desired);
    let mut cluster = Cluster::at(PREVIOUS);
    cluster.never_ready = Some(DataplaneComponentKind::Genesis);

    let status = drive(&upgrade, &mut cluster);

    assert_eq!(status.phase, DataplaneUpgradePhase::RolledBack);
    assert_eq!(status.current_version, None);
    assert_eq!(
        cluster.patched,
        vec![
            DataplaneComponentKind::Herald,
            DataplaneComponentKind::Genesis
        ]
    );
    assert_eq!(
        cluster.restored,
        vec![
            (DataplaneComponentKind::Genesis, PREVIOUS.to_string()),
            (DataplaneComponentKind::Herald, PREVIOUS.to_string()),
        ]
    );
    assert!(cluster.versions.values().all(|version| version == PREVIOUS));
}

#[test]
fn the_envelope_of_a_data_plane_action_has_no_deployment_id_key() {
    let value = upgrade_wire(TARGET);

    let keys = value.as_object().expect("an object");
    assert!(!keys.contains_key("deployment_id"), "{value}");
    assert_eq!(value["routing_key"], "dataplane.upgrade");
    assert_eq!(genesis_event(value).deployment_id, None);
}

#[test]
fn the_envelope_of_a_deployment_action_still_carries_its_deployment_id() {
    let deployment = Uuid::from_u128(9);
    let value = wire(herald_action(
        Some(deployment),
        "deployment.create",
        json!({}),
    ));

    assert_eq!(value["deployment_id"], deployment.to_string());
    assert_eq!(genesis_event(value).deployment_id, Some(deployment));
}

#[test]
fn a_payload_without_components_is_rejected_by_the_api_and_by_genesis() {
    assert!(
        DataplaneUpgradePayload::new(
            ApiDataPlaneId(dataplane()),
            TARGET.to_string(),
            vec![],
            ApiStrategy::Rolling,
            1,
        )
        .is_err()
    );

    let mut payload = api_payload(TARGET, vec![ApiComponent::All], 1);
    payload["components"] = json!([]);

    assert!(DataplaneUpgradePayloadV1::from_value(&payload).is_err());
}

#[test]
fn a_max_unavailable_of_zero_is_rejected_by_the_api_and_by_genesis() {
    assert!(
        DataplaneUpgradePayload::new(
            ApiDataPlaneId(dataplane()),
            TARGET.to_string(),
            vec![ApiComponent::All],
            ApiStrategy::Rolling,
            0,
        )
        .is_err()
    );

    let mut payload = api_payload(TARGET, vec![ApiComponent::All], 1);
    payload["max_unavailable"] = json!(0);

    assert!(DataplaneUpgradePayloadV1::from_value(&payload).is_err());
}

#[test]
fn the_api_payload_is_accepted_by_genesis() {
    let payload = api_payload(TARGET, vec![ApiComponent::All], 3);

    let parsed = DataplaneUpgradePayloadV1::from_value(&payload).expect("a valid payload");

    assert_eq!(parsed.dataplane_id, dataplane());
    assert_eq!(parsed.target_version, TARGET);
    assert_eq!(parsed.max_unavailable, 3);
}

#[tokio::test]
async fn a_duplicate_event_creates_one_upgrade() {
    let upgrades = Arc::new(InMemoryUpgrades::default());
    let handler = handler(&upgrades);

    handler
        .handle(genesis_event(upgrade_wire(TARGET)))
        .await
        .expect("created");
    handler
        .handle(genesis_event(upgrade_wire(TARGET)))
        .await
        .expect("a no-op");

    assert_eq!(upgrades.creations(), 1);
}

#[tokio::test]
async fn a_different_version_while_one_is_in_flight_is_refused() {
    let upgrades = Arc::new(InMemoryUpgrades::default());
    let handler = handler(&upgrades);
    handler
        .handle(genesis_event(upgrade_wire(TARGET)))
        .await
        .expect("created");

    let error = handler
        .handle(genesis_event(upgrade_wire("27.0.0")))
        .await
        .expect_err("one is in flight");

    assert!(error.to_string().contains(TARGET), "{error}");
    assert_eq!(upgrades.creations(), 1);
    assert_eq!(
        upgrades.desired(&dataplane().to_string()).target_version,
        TARGET
    );
}

#[test]
fn genesis_receives_the_routing_key_the_control_plane_gives_a_data_plane_upgrade() {
    let upgrades = Arc::new(InMemoryUpgrades::default());
    let key = handler(&upgrades).routing_key().to_string();

    assert_eq!(key, aether_domain::action::DATAPLANE_UPGRADE_ACTION_TYPE);
    assert!(
        genesis_core::infrastructure::rabbitmq::consumer::is_bound(&key),
        "Herald publishes under '{key}' and the broker drops it unless Genesis' queue is bound to it"
    );
}
