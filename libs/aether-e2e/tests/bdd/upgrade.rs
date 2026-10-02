use std::sync::Arc;

use aether_core::dataplane_upgrade::{
    DataplaneUpgradePayload, InvalidDataplaneUpgrade, UpgradeComponent, UpgradeStrategy,
};
use aether_crds::v1alpha::identity_dataplane_upgrade::{
    DataplaneUpgradePhase, IdentityDataplaneUpgrade, IdentityDataplaneUpgradeStatus,
};
use aether_domain::dataplane::value_objects::DataPlaneId;
use aether_e2e::support::cluster::{Cluster, drive_from, halfway};
use aether_e2e::support::upgrades::{
    InMemoryUpgrades, PREVIOUS, api_payload, dataplane, genesis_event, handler, upgrade_wire_with,
};
use aether_operator_core::domain::dataplane_upgrade::DataplaneComponentKind;
use genesis_core::domain::entities::dataplane_upgrade_payload::DataplaneUpgradePayloadV1;
use genesis_core::domain::error::GenesisError;
use genesis_core::domain::ports::EventHandler;
use genesis_core::infrastructure::kubernetes::dataplane_upgrade::resource;
use serde_json::{Value, json};

#[derive(Debug, cucumber::World)]
#[world(init = Self::new)]
pub struct UpgradeWorld {
    upgrades: Arc<InMemoryUpgrades>,
    cluster: Cluster,
    upgrade: Option<IdentityDataplaneUpgrade>,
    recorded: Option<IdentityDataplaneUpgradeStatus>,
    settled: Option<IdentityDataplaneUpgradeStatus>,
    published: Option<Value>,
    outcome: Option<Result<(), GenesisError>>,
    built: Option<Result<DataplaneUpgradePayload, InvalidDataplaneUpgrade>>,
    parsed: Option<Result<DataplaneUpgradePayloadV1, GenesisError>>,
    bound: Option<bool>,
}

impl UpgradeWorld {
    fn new() -> Self {
        Self {
            upgrades: Arc::default(),
            cluster: Cluster::at(PREVIOUS),
            upgrade: None,
            recorded: None,
            settled: None,
            published: None,
            outcome: None,
            built: None,
            parsed: None,
            bound: None,
        }
    }

    async fn publish(&mut self, target: &str, strategy: UpgradeStrategy) {
        let wire = upgrade_wire_with(target, strategy);
        self.published = Some(wire.clone());
        self.outcome = Some(handler(&self.upgrades).handle(genesis_event(wire)).await);
    }

    fn settled(&self) -> &IdentityDataplaneUpgradeStatus {
        self.settled.as_ref().expect("the upgrade settled")
    }
}

fn parse_components(names: &str) -> Vec<UpgradeComponent> {
    names
        .split(',')
        .map(str::trim)
        .filter_map(|name| match name {
            "Herald" => Some(UpgradeComponent::Herald),
            "Genesis" => Some(UpgradeComponent::Genesis),
            "Operator" => Some(UpgradeComponent::Operator),
            "All" => Some(UpgradeComponent::All),
            _ => None,
        })
        .collect()
}

fn parse_kinds(names: &str) -> Vec<DataplaneComponentKind> {
    names
        .split(',')
        .map(str::trim)
        .map(|name| match name {
            "Herald" => DataplaneComponentKind::Herald,
            "Genesis" => DataplaneComponentKind::Genesis,
            "Operator" => DataplaneComponentKind::Operator,
            other => panic!("unknown component {other}"),
        })
        .collect()
}

fn parse_strategy(name: &str) -> UpgradeStrategy {
    match name {
        "rolling" => UpgradeStrategy::Rolling,
        "canary" => UpgradeStrategy::Canary,
        other => panic!("unknown strategy {other}"),
    }
}

fn parse_phase(name: &str) -> DataplaneUpgradePhase {
    match name {
        "Pending" => DataplaneUpgradePhase::Pending,
        "Completed" => DataplaneUpgradePhase::Completed,
        "Failed" => DataplaneUpgradePhase::Failed,
        "RolledBack" => DataplaneUpgradePhase::RolledBack,
        other => panic!("unknown phase {other}"),
    }
}

#[cucumber::given(expr = "Herald published an upgrade to {string} of Herald, Genesis and Operator")]
async fn already_published(world: &mut UpgradeWorld, target: String) {
    world.publish(&target, UpgradeStrategy::Rolling).await;
    world.outcome.take().expect("an outcome").expect("accepted");
}

#[cucumber::given(expr = "the upgrade finished as {string}")]
fn upgrade_ended(world: &mut UpgradeWorld, phase: String) {
    let succeeded = parse_phase(&phase) == DataplaneUpgradePhase::Completed;
    world.upgrades.finish(&dataplane().to_string(), succeeded);
}

#[cucumber::given(expr = "a data plane running {string}")]
fn running(world: &mut UpgradeWorld, version: String) {
    world.cluster = Cluster::at(&version);
}

#[cucumber::given(
    expr = "a {word} upgrade to {string} of Herald, Genesis and Operator was requested"
)]
async fn requested(world: &mut UpgradeWorld, strategy: String, target: String) {
    world.publish(&target, parse_strategy(&strategy)).await;
    let desired = world.upgrades.desired(&dataplane().to_string());
    world.upgrade = Some(resource(&desired));
}

#[cucumber::given("Genesis never becomes ready on the target version")]
fn never_ready(world: &mut UpgradeWorld) {
    world.cluster.never_ready = Some(DataplaneComponentKind::Genesis);
}

#[cucumber::given("the operator recorded Herald as upgraded and Genesis as upgrading")]
fn recorded_halfway(world: &mut UpgradeWorld) {
    let target = world
        .upgrade
        .as_ref()
        .expect("requested")
        .spec
        .target_version
        .clone();
    world.recorded = Some(halfway(&mut world.cluster, &target, PREVIOUS));
}

#[cucumber::when(
    expr = "an upgrade request for components {string} is built with max_unavailable {int}"
)]
fn request_built(world: &mut UpgradeWorld, names: String, max_unavailable: u32) {
    let components = parse_components(&names);
    let mut base = api_payload("26.1.0", vec![UpgradeComponent::All], 1);
    base["components"] = json!(components);
    base["max_unavailable"] = json!(max_unavailable);
    world.parsed = Some(DataplaneUpgradePayloadV1::from_value(&base));
    world.built = Some(DataplaneUpgradePayload::new(
        DataPlaneId(dataplane()),
        "26.1.0".to_string(),
        components,
        UpgradeStrategy::Rolling,
        max_unavailable,
    ));
}

#[cucumber::when(expr = "Herald publishes an upgrade to {string} of Herald, Genesis and Operator")]
async fn publishes(world: &mut UpgradeWorld, target: String) {
    world.publish(&target, UpgradeStrategy::Rolling).await;
}

#[cucumber::when("the operator reconciles until the upgrade settles")]
fn reconciles(world: &mut UpgradeWorld) {
    let upgrade = world.upgrade.as_ref().expect("an upgrade was requested");
    let status = world.recorded.take().unwrap_or_default();
    world.settled = Some(drive_from(&upgrade.spec, status, &mut world.cluster));
}

#[cucumber::when(expr = "Genesis' queue is asked about the routing key {string}")]
fn asked_about(world: &mut UpgradeWorld, key: String) {
    world.bound = Some(genesis_core::infrastructure::rabbitmq::consumer::is_bound(
        &key,
    ));
}

#[cucumber::then("the control plane accepts the request")]
fn control_plane_accepts(world: &mut UpgradeWorld) {
    let built = world.built.as_ref().expect("a request");
    assert!(built.is_ok(), "{built:?}");
}

#[cucumber::then("the control plane rejects the request")]
fn control_plane_rejects(world: &mut UpgradeWorld) {
    assert!(world.built.as_ref().expect("a request").is_err());
}

#[cucumber::then("Genesis accepts the request")]
fn genesis_accepts(world: &mut UpgradeWorld) {
    let parsed = world.parsed.as_ref().expect("a request");
    assert!(parsed.is_ok(), "{parsed:?}");
}

#[cucumber::then("Genesis rejects the request")]
fn genesis_rejects(world: &mut UpgradeWorld) {
    assert!(world.parsed.as_ref().expect("a request").is_err());
}

#[cucumber::then("Genesis holds one upgrade named after the data plane")]
fn holds_one(world: &mut UpgradeWorld) {
    assert_eq!(world.upgrades.len(), 1);
    let desired = world.upgrades.desired(&dataplane().to_string());
    assert_eq!(desired.name, dataplane().to_string());
}

#[cucumber::then(expr = "the upgrade phase is {string}")]
fn upgrade_is(world: &mut UpgradeWorld, phase: String) {
    let desired = world.upgrades.desired(&dataplane().to_string());
    let status = resource(&desired).status.unwrap_or_default();
    assert_eq!(status.phase, parse_phase(&phase));
}

#[cucumber::then("the action carries no deployment id")]
fn no_deployment_id(world: &mut UpgradeWorld) {
    let wire = world.published.as_ref().expect("a published action");
    assert!(
        !wire
            .as_object()
            .expect("an object")
            .contains_key("deployment_id")
    );
    assert_eq!(genesis_event(wire.clone()).deployment_id, None);
}

#[cucumber::then(expr = "the creation count is {int}")]
fn created_times(world: &mut UpgradeWorld, times: usize) {
    assert_eq!(world.upgrades.creations(), times);
}

#[cucumber::then(expr = "the replacement count is {int}")]
fn replaced_times(world: &mut UpgradeWorld, times: usize) {
    assert_eq!(world.upgrades.replacements(), times);
}

#[cucumber::then("Genesis refuses the action")]
fn refuses(world: &mut UpgradeWorld) {
    let outcome = world.outcome.as_ref().expect("an outcome");
    assert!(outcome.is_err(), "{outcome:?}");
}

#[cucumber::then(expr = "the upgrade targets {string}")]
fn targets(world: &mut UpgradeWorld, version: String) {
    let desired = world.upgrades.desired(&dataplane().to_string());
    assert_eq!(desired.target_version, version);
}

#[cucumber::then(expr = "the upgrade ends {string}")]
fn settled_as(world: &mut UpgradeWorld, phase: String) {
    assert_eq!(world.settled().phase, parse_phase(&phase));
}

#[cucumber::then(expr = "the upgrade is {string} at version {string}")]
fn settled_at(world: &mut UpgradeWorld, phase: String, version: String) {
    let status = world.settled();
    assert_eq!(status.phase, parse_phase(&phase));
    assert_eq!(status.current_version.as_deref(), Some(version.as_str()));
}

#[cucumber::then(expr = "the upgrade is {string} with reason {string}")]
fn settled_with_reason(world: &mut UpgradeWorld, phase: String, reason: String) {
    let status = world.settled();
    assert_eq!(status.phase, parse_phase(&phase));
    assert_eq!(
        status.conditions[0].reason.as_deref(),
        Some(reason.as_str())
    );
}

#[cucumber::then(expr = "the components were patched in the order {string}")]
fn patched_in_order(world: &mut UpgradeWorld, names: String) {
    assert_eq!(world.cluster.patched, parse_kinds(&names));
}

#[cucumber::then(expr = "the components were restored in the order {string}")]
fn restored_in_order(world: &mut UpgradeWorld, names: String) {
    let restored: Vec<_> = world
        .cluster
        .restored
        .iter()
        .map(|(kind, _)| *kind)
        .collect();
    assert_eq!(restored, parse_kinds(&names));
}

#[cucumber::then(expr = "every component runs {string}")]
fn every_runs(world: &mut UpgradeWorld, version: String) {
    assert!(
        world
            .cluster
            .versions
            .values()
            .all(|running| *running == version)
    );
}

#[cucumber::then("no deployment was touched")]
fn untouched(world: &mut UpgradeWorld) {
    assert!(world.cluster.patched.is_empty());
    assert!(world.cluster.restored.is_empty());
}

#[cucumber::then(expr = "the routing key is {word}")]
fn routing_key_is(world: &mut UpgradeWorld, binding: String) {
    assert_eq!(world.bound, Some(binding == "bound"));
}
