use aether_domain::deployments::reachability_history::{
    DowntimeInterval, ReachabilityCheck, UptimeWindow, availability, downtime_intervals,
};
use aether_e2e::support::reachability::{at, check, mark_down, series};
use chrono::{DateTime, Duration, Utc};
use cucumber::{World, given, then, when};

const DAY: i64 = 86_400;

#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct ReachabilityWorld {
    checks: Vec<ReachabilityCheck>,
    now: DateTime<Utc>,
    window: Option<UptimeWindow>,
    intervals: Vec<DowntimeInterval>,
}

impl ReachabilityWorld {
    fn new() -> Self {
        Self {
            checks: Vec::new(),
            now: at(0),
            window: None,
            intervals: Vec::new(),
        }
    }

    fn compute(&mut self, now: DateTime<Utc>, window: Duration) {
        self.now = now;
        self.window = Some(availability(&self.checks, now, window));
    }

    fn computed(&self) -> &UptimeWindow {
        self.window.as_ref().expect("availability was computed")
    }
}

#[given(expr = "a deployment checked every {int} minute(s) for {int} day(s)")]
fn checked_every(world: &mut ReachabilityWorld, minutes: i64, days: i64) {
    world.checks = series(0, days * DAY, minutes * 60, true);
    world.now = at(days * DAY);
}

#[given(expr = "it was down for {int} minutes after {int} days")]
fn was_down(world: &mut ReachabilityWorld, minutes: i64, days: i64) {
    mark_down(&mut world.checks, days * DAY, days * DAY + minutes * 60);
}

#[given(expr = "a reachable check at {int} seconds")]
fn reachable_check(world: &mut ReachabilityWorld, seconds: i64) {
    world.checks.push(check(seconds, true));
}

#[given(expr = "a failed check at {int} seconds")]
fn failed_check(world: &mut ReachabilityWorld, seconds: i64) {
    world.checks.push(check(seconds, false));
}

#[when(expr = "availability is computed over the last {int} days")]
fn computed_over_days(world: &mut ReachabilityWorld, days: i64) {
    world.compute(world.now, Duration::days(days));
}

#[when(expr = "availability is computed at {int} seconds over a window of {int} seconds")]
fn computed_at(world: &mut ReachabilityWorld, now: i64, window: i64) {
    world.compute(at(now), Duration::seconds(window));
}

#[when("downtime intervals are derived")]
fn derived(world: &mut ReachabilityWorld) {
    world.intervals = downtime_intervals(&world.checks);
}

#[then(expr = "availability is {float} percent")]
fn availability_is(world: &mut ReachabilityWorld, percent: f64) {
    let actual = world.computed().uptime_percent.expect("a percentage");
    assert!((actual - percent).abs() < 1e-5, "{actual} is not {percent}");
}

#[then("there is no availability percentage")]
fn no_percentage(world: &mut ReachabilityWorld) {
    assert_eq!(world.computed().uptime_percent, None);
}

#[then(expr = "the window coverage is {word}")]
fn coverage(world: &mut ReachabilityWorld, expected: String) {
    assert_eq!(world.computed().covers_full_window, expected == "full");
}

#[then(expr = "there is/are {int} downtime interval(s)")]
fn interval_count(world: &mut ReachabilityWorld, count: usize) {
    assert_eq!(world.intervals.len(), count, "{:?}", world.intervals);
}

#[then(
    expr = "downtime interval {int} starts at {int} seconds and ends at {int} seconds, lasting {int} seconds"
)]
fn closed_interval(world: &mut ReachabilityWorld, rank: usize, start: i64, end: i64, lasting: i64) {
    let expected = DowntimeInterval {
        started_at: at(start),
        ended_at: Some(at(end)),
        duration_seconds: Some(lasting),
    };
    assert_eq!(world.intervals[rank - 1], expected);
}

#[then(expr = "downtime interval {int} starts at {int} seconds and is still open")]
fn open_interval(world: &mut ReachabilityWorld, rank: usize, start: i64) {
    let expected = DowntimeInterval {
        started_at: at(start),
        ended_at: None,
        duration_seconds: None,
    };
    assert_eq!(world.intervals[rank - 1], expected);
}

#[then(expr = "availability at {int} seconds over a window of {int} seconds has no percentage")]
fn availability_has_no_percentage(world: &mut ReachabilityWorld, now: i64, window: i64) {
    let computed = availability(&world.checks, at(now), Duration::seconds(window));
    assert_eq!(computed.uptime_percent, None);
}
