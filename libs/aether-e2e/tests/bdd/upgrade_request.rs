use std::collections::HashMap;

use aether_core::dataplane_upgrade::{UpgradeComponent, UpgradeStrategy};
use aether_core::dataplane_upgrade_request::{
    DataplaneUpgradeError, DataplaneUpgradeRequest, DataplaneUpgradeRequestService,
    DataplaneUpgrades, RequestedUpgrade,
};
use aether_domain::CoreError;
use aether_domain::action::ActionStatus;
use aether_domain::dataplane::value_objects::DataPlaneId;
use aether_domain::platform::PlatformRight;
use aether_domain::version::Version;
use aether_e2e::support::platform::{HeldRights, caller};
use aether_e2e::support::reachability::at;
use aether_e2e::support::requests::{
    InMemoryActions, InMemoryDataPlanes, failed, is_dataplane_upgrade, published,
};
use cucumber::{World, given, then, when};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug)]
enum Refusal {
    Service(DataplaneUpgradeError),
    UnknownStrategy,
}

impl Refusal {
    fn reason(&self) -> &'static str {
        match self {
            Self::UnknownStrategy => "the payload is invalid",
            Self::Service(error) => match error {
                DataplaneUpgradeError::Invalid(_) | DataplaneUpgradeError::BadVersion(_) => {
                    "the payload is invalid"
                }
                DataplaneUpgradeError::UnknownDataplane(_) => "the data plane is unknown",
                DataplaneUpgradeError::NoMinimumConfigured { .. } => {
                    "no minimum Herald version is configured"
                }
                DataplaneUpgradeError::HeraldTooOld { reported: None, .. } => {
                    "the Herald reports no version"
                }
                DataplaneUpgradeError::HeraldTooOld { .. } => "the Herald is below the minimum",
                DataplaneUpgradeError::AlreadyUpgrading { .. } => {
                    "an upgrade to another version is under way"
                }
                DataplaneUpgradeError::Core(CoreError::MissingPlatformRight { right })
                    if *right == PlatformRight::OperateFleet.to_string() =>
                {
                    "the right to operate the fleet is missing"
                }
                DataplaneUpgradeError::Core(_) => "an unexpected failure",
            },
        }
    }
}

type Outcome = Result<Vec<RequestedUpgrade>, Refusal>;

#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct UpgradeRequestWorld {
    actions: InMemoryActions,
    planes: InMemoryDataPlanes,
    names: HashMap<String, DataPlaneId>,
    rights: Vec<PlatformRight>,
    minimum: Option<Version>,
    earlier: Vec<RequestedUpgrade>,
    outcome: Option<Outcome>,
    listing: Vec<DataplaneUpgrades>,
}

impl UpgradeRequestWorld {
    fn new() -> Self {
        Self {
            actions: InMemoryActions::default(),
            planes: InMemoryDataPlanes::default(),
            names: HashMap::new(),
            rights: Vec::new(),
            minimum: None,
            earlier: Vec::new(),
            outcome: None,
            listing: Vec::new(),
        }
    }

    fn service(
        &self,
    ) -> DataplaneUpgradeRequestService<InMemoryActions, InMemoryDataPlanes, HeldRights> {
        DataplaneUpgradeRequestService::new(
            self.actions.clone(),
            self.planes.clone(),
            HeldRights::of(&self.rights),
        )
    }

    fn id_of(&mut self, name: &str) -> DataPlaneId {
        *self
            .names
            .entry(name.to_string())
            .or_insert_with(|| DataPlaneId(Uuid::new_v4()))
    }

    fn registered(&self, name: &str) -> DataPlaneId {
        self.names[name]
    }

    async fn request(
        &mut self,
        version: &str,
        targets: &str,
        components: Vec<UpgradeComponent>,
        strategy: UpgradeStrategy,
        max_unavailable: u32,
    ) -> Outcome {
        let ids = names_in(targets)
            .into_iter()
            .map(|name| self.id_of(name))
            .collect();
        let request = DataplaneUpgradeRequest {
            target_version: version.to_string(),
            dataplanes: Some(ids),
            components,
            strategy,
            max_unavailable,
        };
        self.service()
            .request(caller(), self.minimum.as_ref(), request)
            .await
            .map_err(Refusal::Service)
    }

    fn refusal(&self) -> &Refusal {
        match self.outcome.as_ref().expect("a request was made") {
            Err(refusal) => refusal,
            Ok(requested) => panic!("the request was accepted: {requested:?}"),
        }
    }

    fn requested(&self) -> &[RequestedUpgrade] {
        match self.outcome.as_ref().expect("a request was made") {
            Ok(requested) => requested,
            Err(refusal) => panic!("the request was refused: {refusal:?}"),
        }
    }
}

fn names_in(list: &str) -> Vec<&str> {
    list.split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect()
}

fn all_components() -> Vec<UpgradeComponent> {
    vec![UpgradeComponent::All]
}

fn component_named(name: &str) -> UpgradeComponent {
    serde_json::from_value(json!(name)).expect("a known component")
}

fn status_name(status: &ActionStatus) -> &'static str {
    match status {
        ActionStatus::Pending => "pending",
        ActionStatus::Published { .. } => "published",
        ActionStatus::Failed { .. } => "failed",
        _ => "in progress",
    }
}

#[given("an operator who holds the right to operate the fleet")]
fn operator(world: &mut UpgradeRequestWorld) {
    world.rights.push(PlatformRight::OperateFleet);
}

#[given("the operator also holds the right to view the estate")]
fn operator_views(world: &mut UpgradeRequestWorld) {
    world.rights.push(PlatformRight::ViewEstate);
}

#[given("a user who holds no platform right")]
fn user_without_rights(world: &mut UpgradeRequestWorld) {
    world.rights.clear();
}

#[given(expr = "the minimum Herald version is {string}")]
fn minimum_version(world: &mut UpgradeRequestWorld, minimum: String) {
    world.minimum = match minimum.as_str() {
        "unset" => None,
        version => Some(Version::parse(version).expect("a version")),
    };
}

#[given(expr = "data planes {string} whose Herald reports {string}")]
fn data_planes(world: &mut UpgradeRequestWorld, names: String, reported: String) {
    let reported = match reported.as_str() {
        "no version" => None,
        version => Some(Version::parse(version).expect("a version")),
    };
    for name in names_in(&names) {
        let id = world.planes.register(reported.clone());
        world.names.insert(name.to_string(), id);
    }
}

#[given(expr = "an upgrade of {string} to {string} was requested")]
async fn upgrade_was_requested(world: &mut UpgradeRequestWorld, name: String, version: String) {
    let outcome = world
        .request(
            &version,
            &name,
            all_components(),
            UpgradeStrategy::Rolling,
            1,
        )
        .await;
    world
        .earlier
        .extend(outcome.expect("the earlier request was accepted"));
}

#[given(expr = "the upgrade of {string} is {word}")]
fn upgrade_finished(world: &mut UpgradeRequestWorld, name: String, finish: String) {
    let status = match finish.as_str() {
        "published" => published(at(0)),
        "failed" => failed(at(0)),
        other => panic!("unknown final status {other}"),
    };
    world.actions.set_status(world.registered(&name), &status);
}

#[when(expr = "the operator requests an upgrade to {string} of the data planes {string}")]
async fn requests_upgrade(world: &mut UpgradeRequestWorld, version: String, targets: String) {
    let outcome = world
        .request(
            &version,
            &targets,
            all_components(),
            UpgradeStrategy::Rolling,
            1,
        )
        .await;
    world.outcome = Some(outcome);
}

#[when(
    expr = "the operator requests an upgrade of the data planes {string} to {string} of components {string} by strategy {string} with max_unavailable {int}"
)]
async fn requests_described_upgrade(
    world: &mut UpgradeRequestWorld,
    targets: String,
    version: String,
    components: String,
    strategy: String,
    max_unavailable: u32,
) {
    let components = names_in(&components)
        .into_iter()
        .map(component_named)
        .collect();
    let outcome = match serde_json::from_value::<UpgradeStrategy>(json!(strategy)) {
        Ok(strategy) => {
            world
                .request(&version, &targets, components, strategy, max_unavailable)
                .await
        }
        Err(_) => Err(Refusal::UnknownStrategy),
    };
    world.outcome = Some(outcome);
}

#[when("the operator lists the data plane upgrades")]
async fn lists_upgrades(world: &mut UpgradeRequestWorld) {
    world.listing = world
        .service()
        .list(caller())
        .await
        .expect("the listing was allowed");
}

#[then(expr = "{int} action(s) is/are created")]
fn actions_created(world: &mut UpgradeRequestWorld, count: usize) {
    let actions = world.actions.all();
    assert_eq!(actions.len(), count, "{actions:?}");
}

#[then("nothing is created")]
fn nothing_created(world: &mut UpgradeRequestWorld) {
    let actions = world.actions.all();
    assert!(actions.is_empty(), "{actions:?}");
}

#[then(expr = "a dataplane.upgrade action addressed to {string} with no deployment exists")]
fn upgrade_action_exists(world: &mut UpgradeRequestWorld, name: String) {
    let actions = world.actions.addressed_to(world.registered(&name));
    assert!(
        actions
            .iter()
            .any(|action| is_dataplane_upgrade(action) && action.deployment_id.is_none()),
        "{actions:?}"
    );
}

#[then(expr = "no action is addressed to {string}")]
fn no_action_addressed(world: &mut UpgradeRequestWorld, name: String) {
    let actions = world.actions.addressed_to(world.registered(&name));
    assert!(actions.is_empty(), "{actions:?}");
}

#[then("the request is accepted")]
fn accepted(world: &mut UpgradeRequestWorld) {
    world.requested();
}

#[then(expr = "the request is refused because {string}")]
fn refused(world: &mut UpgradeRequestWorld, reason: String) {
    let refusal = world.refusal();
    assert_eq!(refusal.reason(), reason, "{refusal:?}");
}

#[then(expr = "the existing action is returned for {string}")]
fn existing_returned(world: &mut UpgradeRequestWorld, name: String) {
    let id = world.registered(&name);
    let returned = world
        .requested()
        .iter()
        .find(|upgrade| upgrade.dataplane_id == id)
        .expect("an upgrade for the data plane");
    let first = world
        .earlier
        .iter()
        .find(|upgrade| upgrade.dataplane_id == id)
        .expect("an earlier upgrade for the data plane");
    assert!(returned.already_requested);
    assert_eq!(returned.action.id, first.action.id);
}

#[then(expr = "a new action is created for {string}")]
fn new_action_created(world: &mut UpgradeRequestWorld, name: String) {
    let id = world.registered(&name);
    let created = world
        .requested()
        .iter()
        .find(|upgrade| upgrade.dataplane_id == id)
        .expect("an upgrade for the data plane");
    assert!(!created.already_requested);
}

#[then(expr = "{string} has {int} upgrade action(s)")]
fn upgrade_action_count(world: &mut UpgradeRequestWorld, name: String, count: usize) {
    let upgrades = world
        .actions
        .addressed_to(world.registered(&name))
        .into_iter()
        .filter(is_dataplane_upgrade)
        .count();
    assert_eq!(upgrades, count);
}

#[then(expr = "the listing shows {int} data plane(s)")]
fn listing_count(world: &mut UpgradeRequestWorld, count: usize) {
    assert_eq!(world.listing.len(), count, "{:?}", world.listing);
}

#[then(expr = "the listing shows {string} with an upgrade {string}")]
fn listing_shows(world: &mut UpgradeRequestWorld, name: String, status: String) {
    let id = world.registered(&name);
    let listed = world
        .listing
        .iter()
        .find(|upgrades| upgrades.dataplane_id == id)
        .expect("the data plane is listed");
    let statuses: Vec<_> = listed
        .actions
        .iter()
        .map(|action| status_name(&action.status))
        .collect();
    assert_eq!(statuses, [status.as_str()]);
}
