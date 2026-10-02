use std::collections::HashMap;

use aether_domain::CoreError;
use aether_domain::dataplane::entities::DataPlane;
use aether_domain::dataplane::value_objects::{
    Capacity, DataPlaneAllocation, DataPlaneLiveness, Region,
};
use aether_domain::deployments::DeploymentId;
use aether_domain::platform::PlatformRight;
use aether_domain::signals::ports::SignalListPage;
use aether_domain::signals::service::SignalServiceImpl;
use aether_domain::signals::{Signal, SignalKind, SignalSubject};
use aether_e2e::support::platform::{HeldRights, caller};
use aether_e2e::support::reachability::at;
use aether_e2e::support::signals::{
    InMemorySignals, heartbeat_stale_key, open_signal, settle_heartbeat_stale_signal,
};
use chrono::{DateTime, Duration, Utc};
use cucumber::{World, given, then, when};
use uuid::Uuid;

#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct SignalsWorld {
    signals: InMemorySignals,
    rights: Vec<PlatformRight>,
    planes: HashMap<String, DataPlane>,
    deployments: HashMap<String, DeploymentId>,
    window: Duration,
    started: DateTime<Utc>,
    now: DateTime<Utc>,
    liveness: Option<DataPlaneLiveness>,
    listing: Option<Result<SignalListPage, CoreError>>,
}

impl SignalsWorld {
    fn new() -> Self {
        Self {
            signals: InMemorySignals::default(),
            rights: Vec::new(),
            planes: HashMap::new(),
            deployments: HashMap::new(),
            window: Duration::zero(),
            started: at(100_000),
            now: at(100_000),
            liveness: None,
            listing: None,
        }
    }

    fn service(&self) -> SignalServiceImpl<InMemorySignals, HeldRights> {
        SignalServiceImpl::new(self.signals.clone(), HeldRights::of(&self.rights))
    }

    fn plane(&self, name: &str) -> &DataPlane {
        &self.planes[name]
    }

    fn register(&mut self, name: &str, last_seen_at: Option<DateTime<Utc>>) {
        let mut plane = DataPlane::new(
            DataPlaneAllocation::Shared,
            Region::new("fr-par"),
            Capacity::new(4_000, 8_192, 100).expect("a non-zero capacity"),
        );
        plane.last_seen_at = last_seen_at;
        self.planes.insert(name.to_string(), plane);
    }

    async fn tick(&self) {
        let service = self.service();
        for plane in self.planes.values() {
            settle_heartbeat_stale_signal(&service, plane, self.now, self.window)
                .await
                .expect("the probe wrote its signal");
        }
    }

    fn name_of(&self, subject: &SignalSubject) -> String {
        match subject {
            SignalSubject::Dataplane { id } => self
                .planes
                .iter()
                .find(|(_, plane)| plane.id == *id)
                .map(|(name, _)| name.clone()),
            SignalSubject::Deployment { id } => self
                .deployments
                .iter()
                .find(|(_, deployment)| *deployment == id)
                .map(|(name, _)| name.clone()),
            SignalSubject::Action { .. } => None,
        }
        .expect("a named subject")
    }

    fn open_about(&self, kind: &str, name: &str) -> Vec<Signal> {
        self.signals
            .open_rows()
            .into_iter()
            .filter(|signal| signal.kind.as_str() == kind && self.name_of(&signal.subject) == name)
            .collect()
    }
}

fn liveness_name(liveness: DataPlaneLiveness) -> &'static str {
    match liveness {
        DataPlaneLiveness::Reachable => "reachable",
        DataPlaneLiveness::Unreachable => "unreachable",
        DataPlaneLiveness::NeverSeen => "never seen",
    }
}

#[given(expr = "a heartbeat window of {int} seconds")]
fn window(world: &mut SignalsWorld, seconds: i64) {
    world.window = Duration::seconds(seconds);
}

#[given("an operator who holds the right to view the estate")]
fn operator(world: &mut SignalsWorld) {
    world.rights.push(PlatformRight::ViewEstate);
}

#[given("a user who holds no platform right")]
fn user_without_rights(world: &mut SignalsWorld) {
    world.rights.clear();
}

#[given(expr = "a data plane {string} that last reported {int} seconds ago")]
fn reported_ago(world: &mut SignalsWorld, name: String, seconds: i64) {
    let seen = world.now - Duration::seconds(seconds);
    world.register(&name, Some(seen));
}

#[given(expr = "a data plane {string} that was never seen")]
fn never_seen(world: &mut SignalsWorld, name: String) {
    world.register(&name, None);
}

#[given(expr = "{int} seconds pass")]
fn time_passes(world: &mut SignalsWorld, seconds: i64) {
    world.now += Duration::seconds(seconds);
}

#[given(expr = "{string} sends a heartbeat")]
fn heartbeat(world: &mut SignalsWorld, name: String) {
    let now = world.now;
    world
        .planes
        .get_mut(&name)
        .expect("a registered data plane")
        .last_seen_at = Some(now);
}

#[given("the heartbeat probe has ticked")]
async fn probe_has_ticked(world: &mut SignalsWorld) {
    world.tick().await;
}

#[given(expr = "a heartbeat stale signal left open for {string}")]
async fn left_open(world: &mut SignalsWorld, name: String) {
    let plane = world.plane(&name);
    let signal = open_signal(
        SignalKind::DataplaneHeartbeatStale,
        SignalSubject::Dataplane { id: plane.id },
        heartbeat_stale_key(plane),
        "left from an earlier silence".to_string(),
        world.started,
    );
    world
        .service()
        .write_signal(signal)
        .await
        .expect("the signal was written");
}

#[when("the heartbeat probe ticks")]
async fn probe_ticks(world: &mut SignalsWorld) {
    world.tick().await;
}

#[when(expr = "the liveness of {string} is derived")]
fn liveness_derived(world: &mut SignalsWorld, name: String) {
    world.liveness = Some(world.plane(&name).liveness(world.now, world.window));
}

#[when("the caller lists the open signals")]
async fn lists_open(world: &mut SignalsWorld) {
    world.listing = Some(
        world
            .service()
            .list_open_signals(caller(), None, None, 100, None)
            .await,
    );
}

#[when(expr = "a {string} signal is written for the deployment {string}")]
async fn signal_written(world: &mut SignalsWorld, kind: String, deployment: String) {
    let id = *world
        .deployments
        .entry(deployment.clone())
        .or_insert_with(|| DeploymentId(Uuid::new_v4()));
    let signal = open_signal(
        kind.parse().expect("a known signal kind"),
        SignalSubject::Deployment { id },
        format!("deployment-unreachable-{}", id.0),
        format!("Deployment {deployment} did not answer"),
        world.now,
    );
    world
        .service()
        .write_signal(signal)
        .await
        .expect("the signal was written");
}

#[then(expr = "there is/are {int} open signal(s)")]
fn open_count(world: &mut SignalsWorld, count: usize) {
    assert_eq!(world.signals.open_rows().len(), count);
}

#[then("there is no open signal")]
fn no_open(world: &mut SignalsWorld) {
    let open = world.signals.open_rows();
    assert!(open.is_empty(), "{open:?}");
}

#[then(expr = "there is/are {int} closed signal(s)")]
fn closed_count(world: &mut SignalsWorld, count: usize) {
    assert_eq!(world.signals.closed_rows().len(), count);
}

#[then(expr = "there is/are {int} signal(s) in all")]
fn total_count(world: &mut SignalsWorld, count: usize) {
    assert_eq!(
        world.signals.rows().len(),
        count,
        "{:?}",
        world.signals.rows()
    );
}

#[then(expr = "the open signal is a {string} about {string}")]
fn open_signal_about(world: &mut SignalsWorld, kind: String, name: String) {
    let found = world.open_about(&kind, &name);
    assert_eq!(found.len(), 1, "{:?}", world.signals.rows());
}

#[then("the open signal was opened at the first tick and last seen 30 seconds later")]
fn opened_then_refreshed(world: &mut SignalsWorld) {
    let open = world.signals.open_rows();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].opened_at, world.started);
    assert_eq!(open[0].last_seen_at, world.started + Duration::seconds(30));
}

#[then("the open signal is not the closed one")]
fn not_the_closed_one(world: &mut SignalsWorld) {
    let open = world.signals.open_rows();
    let closed = world.signals.closed_rows();
    assert_ne!(open[0].id, closed[0].id);
    assert!(open[0].opened_at > closed[0].opened_at);
}

#[then(expr = "{string} is {word}")]
fn liveness_is(world: &mut SignalsWorld, _name: String, expected: String) {
    let liveness = world.liveness.expect("liveness was derived");
    assert_eq!(liveness_name(liveness), expected);
}

#[then(expr = "{string} is never seen")]
fn liveness_never_seen(world: &mut SignalsWorld, _name: String) {
    let liveness = world.liveness.expect("liveness was derived");
    assert_eq!(liveness, DataPlaneLiveness::NeverSeen);
}

#[then(expr = "the listing shows {int} signal(s)")]
fn listing_count(world: &mut SignalsWorld, count: usize) {
    let page = world
        .listing
        .as_ref()
        .expect("signals were listed")
        .as_ref()
        .expect("the listing was allowed");
    assert_eq!(page.signals.len(), count, "{page:?}");
}

#[then(expr = "the listing shows a signal about {string}")]
fn listing_about(world: &mut SignalsWorld, name: String) {
    let page = world
        .listing
        .as_ref()
        .expect("signals were listed")
        .as_ref()
        .expect("the listing was allowed");
    assert!(
        page.signals
            .iter()
            .any(|signal| world.name_of(&signal.subject) == name),
        "{page:?}"
    );
}

#[then("the listing is refused because the right to view the estate is missing")]
fn listing_refused(world: &mut SignalsWorld) {
    let right = PlatformRight::ViewEstate.to_string();
    match world.listing.as_ref().expect("signals were listed") {
        Err(CoreError::MissingPlatformRight { right: missing }) => assert_eq!(*missing, right),
        other => panic!("the listing was not refused for lack of the right: {other:?}"),
    }
}
