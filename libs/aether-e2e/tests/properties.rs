use std::collections::BTreeMap;
use std::time::Duration;

use aether_core::dataplane_upgrade::{
    DataplaneUpgradePayload, UpgradeComponent as ApiComponent, UpgradeStrategy as ApiStrategy,
};
use aether_crds::v1alpha::identity_dataplane_upgrade::{
    ComponentUpgradeState, DataplaneComponent, DataplaneUpgradePhase, DataplaneUpgradeStrategy,
    IdentityDataplaneUpgradeSpec, IdentityDataplaneUpgradeStatus,
};
use aether_domain::dataplane::value_objects::DataPlaneId;
use aether_domain::deployments::reachability_history::{
    MAX_GAP, ReachabilityCheck, availability, downtime_intervals,
};
use aether_e2e::support::reachability::{at, check};
use aether_operator_core::domain::dataplane_upgrade::service::{
    Observed, Step, advance, decide, preflight,
};
use aether_operator_core::domain::dataplane_upgrade::{ComponentVersions, DataplaneComponentKind};
use chrono::{DateTime, Duration as Delta, Utc};
use genesis_core::domain::entities::dataplane_upgrade_payload::{
    DataplaneUpgradePayloadV1, UpgradeComponent as GenesisComponent,
    UpgradeStrategy as GenesisStrategy,
};
use genesis_core::infrastructure::rabbitmq::consumer::is_bound;
use proptest::prelude::*;
use uuid::Uuid;

const WIDE_WINDOW: i64 = 10_000_000;
const EPSILON: f64 = 1e-9;

fn max_gap_seconds() -> i64 {
    MAX_GAP.num_seconds()
}

fn inner_gap() -> impl Strategy<Value = i64> {
    prop_oneof![
        3 => 1..max_gap_seconds(),
        1 => Just(max_gap_seconds()),
        3 => (max_gap_seconds() + 1)..=2_000,
    ]
}

fn tail_gap() -> impl Strategy<Value = i64> {
    prop_oneof![Just(0), inner_gap()]
}

#[derive(Debug, Clone)]
struct Series {
    times: Vec<i64>,
    reachable: Vec<bool>,
    now: i64,
}

impl Series {
    fn checks(&self) -> Vec<ReachabilityCheck> {
        self.times
            .iter()
            .zip(&self.reachable)
            .map(|(seconds, reachable)| check(*seconds, *reachable))
            .collect()
    }

    fn shifted_from(&self, index: usize, by: i64) -> Self {
        let mut shifted = self.clone();
        for time in &mut shifted.times[index..] {
            *time += by;
        }
        shifted.now += by;
        shifted
    }

    fn percent(&self, window: i64) -> Option<f64> {
        availability(&self.checks(), at(self.now), Delta::seconds(window)).uptime_percent
    }
}

fn series() -> impl Strategy<Value = Series> {
    (
        any::<bool>(),
        prop::collection::vec((inner_gap(), any::<bool>()), 0..40),
        tail_gap(),
    )
        .prop_map(|(first, rest, tail)| {
            let mut times = vec![0];
            let mut reachable = vec![first];
            for (gap, state) in rest {
                times.push(times[times.len() - 1] + gap);
                reachable.push(state);
            }
            let now = times[times.len() - 1] + tail;
            Series {
                times,
                reachable,
                now,
            }
        })
}

fn reachable_series() -> impl Strategy<Value = Series> {
    series().prop_map(|mut series| {
        series.reachable.iter_mut().for_each(|state| *state = true);
        series
    })
}

fn not_above(after: Option<f64>, before: Option<f64>) -> bool {
    match (after, before) {
        (Some(after), Some(before)) => after <= before + EPSILON,
        (None, None) => true,
        _ => false,
    }
}

proptest! {
    #[test]
    fn availability_is_a_percentage_in_range(series in series(), window in 1..=WIDE_WINDOW) {
        if let Some(percent) = series.percent(window) {
            prop_assert!((0.0..=100.0).contains(&percent), "{percent}");
        }
    }

    #[test]
    fn availability_of_a_series_without_failures_is_full_or_unobserved(
        series in reachable_series(),
        window in 1..=WIDE_WINDOW,
    ) {
        let percent = series.percent(window);
        prop_assert!(percent.is_none() || percent == Some(100.0), "{percent:?}");
    }

    #[test]
    fn making_a_check_unreachable_never_raises_availability(
        series in series(),
        window in 1..=WIDE_WINDOW,
        pick in any::<prop::sample::Index>(),
    ) {
        let reachable: Vec<usize> = (0..series.times.len())
            .filter(|index| series.reachable[*index])
            .collect();
        prop_assume!(!reachable.is_empty());
        let index = reachable[pick.index(reachable.len())];
        let mut degraded = series.clone();
        degraded.reachable[index] = false;

        let before = series.percent(window);
        let after = degraded.percent(window);

        prop_assert!(not_above(after, before), "before {before:?} after {after:?}");
    }

    #[test]
    fn a_gap_that_is_already_unobserved_can_grow_without_changing_availability(
        series in series(),
        window in Just(WIDE_WINDOW),
        by in (max_gap_seconds() + 1)..=5_000,
    ) {
        let long: Vec<usize> = (1..series.times.len())
            .filter(|index| series.times[*index] - series.times[*index - 1] > max_gap_seconds())
            .collect();
        prop_assume!(!long.is_empty());

        for index in long {
            let grown = series.shifted_from(index, by);
            prop_assert_eq!(grown.percent(window), series.percent(window));
        }
    }

    #[test]
    fn a_gap_inserted_after_a_reachable_check_never_raises_availability(
        series in series(),
        window in Just(WIDE_WINDOW),
        by in (max_gap_seconds() + 1)..=5_000,
        pick in any::<prop::sample::Index>(),
    ) {
        let reachable: Vec<usize> = (0..series.times.len() - 1)
            .filter(|index| series.reachable[*index])
            .collect();
        prop_assume!(!reachable.is_empty());
        let index = reachable[pick.index(reachable.len())] + 1;
        let grown = series.shifted_from(index, by);

        let before = series.percent(window);
        let after = grown.percent(window);

        prop_assert!(
            after.is_none() || before.is_some_and(|before| after.unwrap() <= before + EPSILON),
            "before {before:?} after {after:?}"
        );
    }

    #[test]
    fn shifting_every_timestamp_and_now_leaves_availability_unchanged(
        series in series(),
        window in 1..=WIDE_WINDOW,
        offset in -1_000_000_000_i64..=1_000_000_000,
    ) {
        let checks = series.checks();
        let moved: Vec<ReachabilityCheck> = series
            .times
            .iter()
            .zip(&series.reachable)
            .map(|(seconds, reachable)| check(seconds + offset, *reachable))
            .collect();

        let before = availability(&checks, at(series.now), Delta::seconds(window));
        let after = availability(&moved, at(series.now + offset), Delta::seconds(window));

        prop_assert_eq!(before, after);
    }

    #[test]
    fn availability_of_no_check_is_unobserved(now in 0_i64..1_000_000_000, window in 1..=WIDE_WINDOW) {
        let result = availability(&[], at(now), Delta::seconds(window));
        prop_assert_eq!(result.uptime_percent, None);
        prop_assert!(!result.covers_full_window);
    }
}

fn check_states() -> impl Strategy<Value = Vec<ReachabilityCheck>> {
    prop::collection::vec((1_i64..=2_000, any::<bool>()), 0..60).prop_map(|steps| {
        let mut seconds = 0;
        steps
            .into_iter()
            .map(|(gap, reachable)| {
                seconds += gap;
                check(seconds, reachable)
            })
            .collect()
    })
}

proptest! {
    #[test]
    fn downtime_intervals_are_ordered_and_disjoint(checks in check_states()) {
        let intervals = downtime_intervals(&checks);

        for pair in intervals.windows(2) {
            let ended = pair[0].ended_at;
            prop_assert!(ended.is_some_and(|ended| ended <= pair[1].started_at), "{pair:?}");
            prop_assert!(pair[0].started_at < pair[1].started_at);
        }
    }

    #[test]
    fn closed_downtime_intervals_run_forward_and_state_their_duration(checks in check_states()) {
        for interval in downtime_intervals(&checks) {
            if let Some(ended) = interval.ended_at {
                prop_assert!(interval.started_at < ended, "{interval:?}");
                prop_assert_eq!(
                    interval.duration_seconds,
                    Some((ended - interval.started_at).num_seconds())
                );
            } else {
                prop_assert_eq!(interval.duration_seconds, None);
            }
        }
    }

    #[test]
    fn at_most_the_last_downtime_interval_is_open(checks in check_states()) {
        let intervals = downtime_intervals(&checks);
        let open: Vec<usize> = intervals
            .iter()
            .enumerate()
            .filter(|(_, interval)| interval.ended_at.is_none())
            .map(|(index, _)| index)
            .collect();

        prop_assert!(open.is_empty() || open == vec![intervals.len() - 1], "{intervals:?}");
    }

    #[test]
    fn downtime_intervals_never_outnumber_the_failed_checks(checks in check_states()) {
        let failed = checks.iter().filter(|check| !check.reachable).count();
        let intervals = downtime_intervals(&checks);

        prop_assert!(intervals.len() <= failed);
        if failed == 0 {
            prop_assert!(intervals.is_empty());
        }
    }

    #[test]
    fn a_downtime_interval_starts_at_the_first_failure_after_a_good_check(checks in check_states()) {
        let intervals = downtime_intervals(&checks);

        for interval in &intervals {
            let index = checks
                .iter()
                .position(|check| check.checked_at == interval.started_at);
            prop_assert!(index.is_some(), "{interval:?}");
            let index = index.unwrap();
            prop_assert!(!checks[index].reachable);
            prop_assert!(index == 0 || checks[index - 1].reachable);
        }

        let starts = (0..checks.len())
            .filter(|index| !checks[*index].reachable && (*index == 0 || checks[*index - 1].reachable))
            .count();
        prop_assert_eq!(intervals.len(), starts);
    }
}

fn word() -> impl Strategy<Value = String> {
    "[a-z]{1,8}"
}

fn more_words() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(word(), 0..4)
}

proptest! {
    #[test]
    fn deployment_and_data_plane_keys_are_bound(
        namespace in prop::sample::select(vec!["deployment", "dataplane"]),
        first in word(),
        rest in more_words(),
    ) {
        let key = std::iter::once(namespace.to_string())
            .chain(std::iter::once(first))
            .chain(rest)
            .collect::<Vec<_>>()
            .join(".");

        prop_assert!(is_bound(&key), "{key}");
    }

    #[test]
    fn keys_of_any_other_namespace_are_not_bound(
        namespace in word().prop_filter("a bound namespace", |word| {
            word != "deployment" && word != "dataplane"
        }),
        first in word(),
        rest in more_words(),
    ) {
        let key = std::iter::once(namespace)
            .chain(std::iter::once(first))
            .chain(rest)
            .collect::<Vec<_>>()
            .join(".");

        prop_assert!(!is_bound(&key), "{key}");
    }
}

fn api_component() -> impl Strategy<Value = ApiComponent> {
    prop::sample::select(vec![
        ApiComponent::Herald,
        ApiComponent::Genesis,
        ApiComponent::Operator,
        ApiComponent::All,
    ])
}

fn api_strategy() -> impl Strategy<Value = ApiStrategy> {
    prop::sample::select(vec![ApiStrategy::Rolling, ApiStrategy::Canary])
}

fn same_component(api: ApiComponent, genesis: GenesisComponent) -> bool {
    matches!(
        (api, genesis),
        (ApiComponent::Herald, GenesisComponent::Herald)
            | (ApiComponent::Genesis, GenesisComponent::Genesis)
            | (ApiComponent::Operator, GenesisComponent::Operator)
            | (ApiComponent::All, GenesisComponent::All)
    )
}

fn same_strategy(api: ApiStrategy, genesis: GenesisStrategy) -> bool {
    matches!(
        (api, genesis),
        (ApiStrategy::Rolling, GenesisStrategy::Rolling)
            | (ApiStrategy::Canary, GenesisStrategy::Canary)
    )
}

fn valid_json(
    dataplane: u128,
    version: &str,
    strategy: ApiStrategy,
    max_unavailable: u32,
) -> serde_json::Value {
    DataplaneUpgradePayload::new(
        DataPlaneId(Uuid::from_u128(dataplane)),
        version.to_string(),
        vec![ApiComponent::All],
        strategy,
        max_unavailable,
    )
    .expect("a valid payload")
    .into_action_payload()
    .data
}

proptest! {
    #[test]
    fn what_the_api_builds_genesis_reads_field_for_field(
        dataplane in any::<u128>(),
        version in "\\PC{0,24}",
        components in prop::collection::vec(api_component(), 1..6),
        strategy in api_strategy(),
        max_unavailable in 1_u32..=10_000,
    ) {
        let json = DataplaneUpgradePayload::new(
            DataPlaneId(Uuid::from_u128(dataplane)),
            version.clone(),
            components.clone(),
            strategy,
            max_unavailable,
        )
        .expect("a valid payload")
        .into_action_payload()
        .data;

        let parsed = DataplaneUpgradePayloadV1::from_value(&json);
        prop_assert!(parsed.is_ok(), "{json}");
        let parsed = parsed.unwrap();

        prop_assert_eq!(parsed.dataplane_id, Uuid::from_u128(dataplane));
        prop_assert_eq!(parsed.target_version, version);
        prop_assert_eq!(parsed.max_unavailable, max_unavailable);
        prop_assert!(same_strategy(strategy, parsed.strategy));
        prop_assert_eq!(parsed.components.len(), components.len());
        for (api, genesis) in components.iter().zip(&parsed.components) {
            prop_assert!(same_component(*api, *genesis), "{api:?} read as {genesis:?}");
        }
    }

    #[test]
    fn a_payload_without_components_is_rejected_on_both_sides(
        dataplane in any::<u128>(),
        version in "\\PC{0,24}",
        strategy in api_strategy(),
        max_unavailable in 1_u32..=10_000,
    ) {
        let built = DataplaneUpgradePayload::new(
            DataPlaneId(Uuid::from_u128(dataplane)),
            version.clone(),
            vec![],
            strategy,
            max_unavailable,
        );
        prop_assert!(built.is_err());

        let mut json = valid_json(dataplane, &version, strategy, max_unavailable);
        json["components"] = serde_json::json!([]);
        prop_assert!(DataplaneUpgradePayloadV1::from_value(&json).is_err(), "{json}");
    }

    #[test]
    fn a_payload_that_lets_nothing_be_unavailable_is_rejected_on_both_sides(
        dataplane in any::<u128>(),
        version in "\\PC{0,24}",
        components in prop::collection::vec(api_component(), 1..6),
        strategy in api_strategy(),
    ) {
        let built = DataplaneUpgradePayload::new(
            DataPlaneId(Uuid::from_u128(dataplane)),
            version.clone(),
            components,
            strategy,
            0,
        );
        prop_assert!(built.is_err());

        let mut json = valid_json(dataplane, &version, strategy, 1);
        json["max_unavailable"] = serde_json::json!(0);
        prop_assert!(DataplaneUpgradePayloadV1::from_value(&json).is_err(), "{json}");
    }
}

const TICK: i64 = 10;
const TIMEOUT: Duration = Duration::from_secs(300);
const TARGET: &str = "target";
const STEP_BOUND: usize = 120;

#[derive(Debug, Clone, Copy)]
enum Outcome {
    Quick,
    ReadyAfter(i64),
    Never,
}

#[derive(Debug)]
struct Trace {
    status: IdentityDataplaneUpgradeStatus,
    settled: bool,
    patched: Vec<DataplaneComponentKind>,
    started: Vec<(DataplaneComponentKind, String)>,
    restored: Vec<(DataplaneComponentKind, String)>,
    operator_patched_early: bool,
    versions: ComponentVersions,
    stayed_terminal: bool,
}

fn old_version(kind: DataplaneComponentKind) -> String {
    format!("{kind}-old")
}

fn start_instant() -> DateTime<Utc> {
    DateTime::from_timestamp(1_800_000_000, 0).expect("a valid timestamp")
}

fn is_upgraded(status: &IdentityDataplaneUpgradeStatus, kind: DataplaneComponentKind) -> bool {
    status.components.iter().any(|entry| {
        entry.name == DataplaneComponent::from(kind)
            && entry.state == ComponentUpgradeState::Upgraded
    })
}

fn run(
    spec: &IdentityDataplaneUpgradeSpec,
    outcomes: &BTreeMap<DataplaneComponentKind, Outcome>,
) -> Trace {
    let selected = DataplaneComponentKind::expand(&spec.components);
    let mut versions: ComponentVersions = DataplaneComponentKind::ALL
        .into_iter()
        .map(|kind| (kind, old_version(kind)))
        .collect();
    let mut ready_at: BTreeMap<DataplaneComponentKind, Option<i64>> = BTreeMap::new();
    let mut status = IdentityDataplaneUpgradeStatus::default();
    let mut trace = Trace {
        status: status.clone(),
        settled: false,
        patched: Vec::new(),
        started: Vec::new(),
        restored: Vec::new(),
        operator_patched_early: false,
        versions: ComponentVersions::new(),
        stayed_terminal: true,
    };

    for tick in 0..STEP_BOUND as i64 {
        let now = start_instant() + Delta::seconds(tick * TICK);
        let ready = DataplaneComponentKind::ALL
            .into_iter()
            .map(|kind| {
                let state = match ready_at.get(&kind) {
                    None => true,
                    Some(when) => when.is_some_and(|when| tick >= when),
                };
                (kind, state)
            })
            .collect();
        let observed = Observed {
            versions: versions.clone(),
            ready,
            now,
        };
        let step =
            preflight(spec, &status).unwrap_or_else(|| decide(spec, &status, &observed, TIMEOUT));

        match &step {
            Step::Start {
                component,
                previous,
            } => trace.started.push((*component, previous.clone())),
            Step::Patch(kind) => {
                if *kind == DataplaneComponentKind::Operator
                    && selected
                        .iter()
                        .filter(|other| **other != DataplaneComponentKind::Operator)
                        .any(|other| !is_upgraded(&status, *other))
                {
                    trace.operator_patched_early = true;
                }
                trace.patched.push(*kind);
                versions.insert(*kind, TARGET.to_string());
                ready_at.insert(
                    *kind,
                    match outcomes[kind] {
                        Outcome::Quick => Some(tick),
                        Outcome::ReadyAfter(ticks) => Some(tick + ticks),
                        Outcome::Never => None,
                    },
                );
            }
            Step::RollBack { restore, .. } => {
                for (kind, version) in restore {
                    versions.insert(*kind, version.clone());
                    ready_at.remove(kind);
                    trace.restored.push((*kind, version.clone()));
                }
            }
            _ => {}
        }

        let was_terminal = matches!(step, Step::Idle);
        let next = advance(spec, &status, &step, now);
        if was_terminal {
            trace.stayed_terminal &= next == status;
            status = next;
            trace.settled = true;
            break;
        }
        status = next;
    }

    trace.status = status;
    trace.versions = versions;
    trace
}

fn settle_further(spec: &IdentityDataplaneUpgradeSpec, trace: &mut Trace) {
    for extra in 1..=5 {
        let now = start_instant() + Delta::seconds((STEP_BOUND as i64 + extra) * TICK);
        let observed = Observed {
            versions: trace.versions.clone(),
            ready: DataplaneComponentKind::ALL
                .into_iter()
                .map(|kind| (kind, extra % 2 == 0))
                .collect(),
            now,
        };
        let step = preflight(spec, &trace.status)
            .unwrap_or_else(|| decide(spec, &trace.status, &observed, TIMEOUT));
        let next = advance(spec, &trace.status, &step, now);
        trace.stayed_terminal &= step == Step::Idle && next == trace.status;
        trace.status = next;
    }
}

fn spec_of(
    components: Vec<DataplaneComponent>,
    strategy: DataplaneUpgradeStrategy,
) -> IdentityDataplaneUpgradeSpec {
    IdentityDataplaneUpgradeSpec {
        dataplane_id: "dp".to_string(),
        target_version: TARGET.to_string(),
        components,
        strategy,
        max_unavailable: 1,
    }
}

fn components() -> impl Strategy<Value = Vec<DataplaneComponent>> {
    prop::collection::vec(
        prop::sample::select(vec![
            DataplaneComponent::Herald,
            DataplaneComponent::Genesis,
            DataplaneComponent::Operator,
            DataplaneComponent::All,
        ]),
        1..6,
    )
}

fn outcome() -> impl Strategy<Value = Outcome> {
    prop_oneof![
        4 => Just(Outcome::Quick),
        3 => (1_i64..=40).prop_map(Outcome::ReadyAfter),
        1 => Just(Outcome::Never),
    ]
}

fn outcomes() -> impl Strategy<Value = BTreeMap<DataplaneComponentKind, Outcome>> {
    prop::collection::vec(outcome(), 3)
        .prop_map(|list| DataplaneComponentKind::ALL.into_iter().zip(list).collect())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn an_upgrade_reaches_a_terminal_phase_within_a_bounded_number_of_steps(
        components in components(),
        outcomes in outcomes(),
    ) {
        let spec = spec_of(components, DataplaneUpgradeStrategy::Rolling);
        let trace = run(&spec, &outcomes);

        prop_assert!(trace.settled, "not settled after {STEP_BOUND} steps: {:?}", trace.status);
        prop_assert!(matches!(
            trace.status.phase,
            DataplaneUpgradePhase::Completed
                | DataplaneUpgradePhase::RolledBack
                | DataplaneUpgradePhase::Failed
        ));
    }

    #[test]
    fn components_are_patched_in_upgrade_order_and_the_operator_last(
        components in components(),
        outcomes in outcomes(),
    ) {
        let spec = spec_of(components, DataplaneUpgradeStrategy::Rolling);
        let selected = DataplaneComponentKind::expand(&spec.components);
        let trace = run(&spec, &outcomes);

        prop_assert!(
            trace.patched.windows(2).all(|pair| pair[0] < pair[1]),
            "{:?}",
            trace.patched
        );
        prop_assert!(trace.patched.iter().all(|kind| selected.contains(kind)));
        prop_assert!(!trace.operator_patched_early);
    }

    #[test]
    fn a_rollback_restores_exactly_the_started_components_in_reverse_order(
        components in components(),
        outcomes in outcomes(),
    ) {
        let spec = spec_of(components, DataplaneUpgradeStrategy::Rolling);
        let trace = run(&spec, &outcomes);
        prop_assume!(trace.status.phase == DataplaneUpgradePhase::RolledBack);

        let expected: Vec<(DataplaneComponentKind, String)> =
            trace.started.iter().rev().cloned().collect();
        prop_assert_eq!(&trace.restored, &expected);
        for kind in DataplaneComponentKind::ALL {
            prop_assert_eq!(&trace.versions[&kind], &old_version(kind));
        }
        prop_assert_eq!(trace.status.current_version, None);
    }

    #[test]
    fn a_completed_upgrade_runs_the_target_everywhere_it_was_asked(
        components in components(),
        outcomes in outcomes(),
    ) {
        let spec = spec_of(components, DataplaneUpgradeStrategy::Rolling);
        let selected = DataplaneComponentKind::expand(&spec.components);
        let trace = run(&spec, &outcomes);
        prop_assume!(trace.status.phase == DataplaneUpgradePhase::Completed);

        prop_assert_eq!(trace.status.current_version.as_deref(), Some(TARGET));
        for kind in DataplaneComponentKind::ALL {
            if selected.contains(&kind) {
                prop_assert!(is_upgraded(&trace.status, kind), "{kind} not upgraded");
                prop_assert_eq!(&trace.versions[&kind], TARGET);
            } else {
                prop_assert_eq!(&trace.versions[&kind], &old_version(kind));
            }
        }
    }

    #[test]
    fn a_canary_spec_fails_without_patching_anything(
        components in components(),
        outcomes in outcomes(),
    ) {
        let spec = spec_of(components, DataplaneUpgradeStrategy::Canary);
        let trace = run(&spec, &outcomes);

        prop_assert_eq!(trace.status.phase, DataplaneUpgradePhase::Failed);
        prop_assert!(trace.patched.is_empty());
        prop_assert!(trace.started.is_empty());
    }

    #[test]
    fn a_terminal_status_never_changes_again(
        components in components(),
        outcomes in outcomes(),
        canary in any::<bool>(),
    ) {
        let strategy = if canary {
            DataplaneUpgradeStrategy::Canary
        } else {
            DataplaneUpgradeStrategy::Rolling
        };
        let spec = spec_of(components, strategy);
        let mut trace = run(&spec, &outcomes);
        prop_assume!(trace.settled);
        let settled = trace.status.clone();

        settle_further(&spec, &mut trace);

        prop_assert!(trace.stayed_terminal);
        prop_assert_eq!(trace.status, settled);
    }
}

mod iam_branding {
    use aether_domain::iam_settings::branding::{Branding, BrandingColorsInput, BrandingInput};
    use aether_e2e::support::iam::{
        domain_colors, resource_branding_after_genesis, resource_colors,
    };
    use aether_operator_core::domain::identity_instance::theme::theme_config;
    use proptest::prelude::*;
    use serde_json::Value;

    const COLOR_KEYS: [&str; 7] = [
        "primaryButton",
        "primaryButtonLabel",
        "links",
        "pageBackground",
        "widgetBackground",
        "bodyText",
        "error",
    ];
    const RADIUS_KEYS: [&str; 3] = ["widgetRadius", "buttonRadius", "inputRadius"];

    fn color() -> impl Strategy<Value = Option<String>> {
        proptest::option::of("#[0-9a-fA-F]{6}")
    }

    fn branding() -> impl Strategy<Value = Branding> {
        (
            [
                color(),
                color(),
                color(),
                color(),
                color(),
                color(),
                color(),
            ],
            proptest::option::of(0i64..=24),
        )
            .prop_map(
                |(
                    [
                        primary,
                        primary_text,
                        links,
                        page_background,
                        widget_background,
                        text,
                        error,
                    ],
                    radius,
                )| {
                    Branding::try_from(BrandingInput {
                        colors: BrandingColorsInput {
                            primary,
                            primary_text,
                            links,
                            page_background,
                            widget_background,
                            text,
                            error,
                        },
                        radius,
                    })
                    .expect("generated values are valid")
                },
            )
    }

    fn is_hex(value: &Value) -> bool {
        value.as_str().is_some_and(|color| {
            color.len() == 7
                && color.starts_with('#')
                && color[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        #[test]
        fn what_the_control_plane_sends_genesis_writes_with_the_same_colors_and_radius(
            branding in branding(),
        ) {
            let written = resource_branding_after_genesis(&branding)
                .unwrap_or_else(|| panic!("Genesis wrote no branding for {branding:?}"));

            let expected = domain_colors(&branding).map(|color| color.map(|c| c.to_lowercase()));
            let colors = resource_colors(&written).map(|color| color.map(|c| c.to_lowercase()));
            prop_assert_eq!(colors, expected);
            prop_assert_eq!(written.radius, branding.radius.map(|radius| radius.get()));
        }

        #[test]
        fn the_theme_holds_only_hex_colors_and_the_three_radius_keys(branding in branding()) {
            let written = resource_branding_after_genesis(&branding).expect("a branding");

            let config = theme_config(&written);

            let object = config.as_object().expect("an object");
            for (group, content) in object {
                let content = content.as_object().expect("a group");
                prop_assert!(!content.is_empty());
                match group.as_str() {
                    "colors" => {
                        for (key, value) in content {
                            prop_assert!(COLOR_KEYS.contains(&key.as_str()), "{key}");
                            prop_assert!(is_hex(value), "{value}");
                        }
                    }
                    "borders" => {
                        let mut keys: Vec<&str> = content.keys().map(String::as_str).collect();
                        keys.sort_unstable();
                        let mut expected = RADIUS_KEYS.to_vec();
                        expected.sort_unstable();
                        prop_assert_eq!(keys, expected);
                        for value in content.values() {
                            prop_assert_eq!(value.as_u64(), branding.radius.map(|r| u64::from(r.get())));
                        }
                    }
                    other => prop_assert!(false, "unexpected group {other}"),
                }
            }

            let empty = domain_colors(&branding).iter().all(Option::is_none)
                && branding.radius.is_none();
            prop_assert_eq!(object.is_empty(), empty);
        }
    }
}
