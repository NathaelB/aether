use std::collections::BTreeMap;
use std::time::Duration;

use aether_crds::common::types::{Condition, ConditionStatus};
use aether_crds::v1alpha::identity_dataplane_upgrade::{
    ComponentUpgradeState, ComponentUpgradeStatus, DataplaneComponent, DataplaneUpgradePhase,
    DataplaneUpgradeStrategy, IdentityDataplaneUpgradeSpec, IdentityDataplaneUpgradeStatus,
};
use chrono::{DateTime, SecondsFormat, Utc};

use super::entities::{ComponentVersions, DataplaneComponentKind};

const ACCEPTED: &str = "Accepted";
const SUCCEEDED: &str = "Succeeded";

#[derive(Debug, Clone)]
pub struct Observed {
    pub versions: ComponentVersions,
    pub ready: BTreeMap<DataplaneComponentKind, bool>,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    CanaryNotSupported,
    NoComponents,
    ComponentMissing(DataplaneComponentKind),
    ReadyTimeout(DataplaneComponentKind),
    RollbackFailed {
        component: DataplaneComponentKind,
        message: String,
    },
}

impl Failure {
    fn condition_type(&self) -> &'static str {
        match self {
            Self::CanaryNotSupported | Self::NoComponents => ACCEPTED,
            _ => SUCCEEDED,
        }
    }

    fn reason(&self) -> &'static str {
        match self {
            Self::CanaryNotSupported => "CanaryNotSupported",
            Self::NoComponents => "NoComponents",
            Self::ComponentMissing(_) => "ComponentNotFound",
            Self::ReadyTimeout(_) => "ReadyTimeout",
            Self::RollbackFailed { .. } => "RollbackFailed",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::CanaryNotSupported => {
                "strategy canary is not implemented, only rolling is supported".to_string()
            }
            Self::NoComponents => "spec.components selects no component".to_string(),
            Self::ComponentMissing(component) => {
                format!("no deployment with a version tag was found for {component}")
            }
            Self::ReadyTimeout(component) => {
                format!("{component} was not ready on the target version before the timeout")
            }
            Self::RollbackFailed { component, message } => {
                format!("restoring {component} failed: {message}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Idle,
    Wait,
    Start {
        component: DataplaneComponentKind,
        previous: String,
    },
    Patch(DataplaneComponentKind),
    MarkUpgraded(DataplaneComponentKind),
    Complete,
    RollBack {
        cause: Failure,
        restore: Vec<(DataplaneComponentKind, String)>,
    },
    Fail(Failure),
}

pub fn preflight(
    spec: &IdentityDataplaneUpgradeSpec,
    status: &IdentityDataplaneUpgradeStatus,
) -> Option<Step> {
    if is_terminal(status.phase) {
        return Some(Step::Idle);
    }
    if spec.strategy == DataplaneUpgradeStrategy::Canary {
        return Some(Step::Fail(Failure::CanaryNotSupported));
    }
    if DataplaneComponentKind::expand(&spec.components).is_empty() {
        return Some(Step::Fail(Failure::NoComponents));
    }
    None
}

pub fn decide(
    spec: &IdentityDataplaneUpgradeSpec,
    status: &IdentityDataplaneUpgradeStatus,
    observed: &Observed,
    timeout: Duration,
) -> Step {
    for component in DataplaneComponentKind::expand(&spec.components) {
        let entry = entry_for(status, component);
        match entry.map(|entry| entry.state) {
            Some(ComponentUpgradeState::Upgraded | ComponentUpgradeState::RolledBack) => continue,
            Some(ComponentUpgradeState::Upgrading) => {
                return decide_upgrading(spec, status, observed, timeout, component);
            }
            Some(ComponentUpgradeState::Pending) | None => {
                return match observed.versions.get(&component) {
                    Some(previous) => Step::Start {
                        component,
                        previous: previous.clone(),
                    },
                    None => abort(status, Failure::ComponentMissing(component)),
                };
            }
        }
    }
    Step::Complete
}

fn decide_upgrading(
    spec: &IdentityDataplaneUpgradeSpec,
    status: &IdentityDataplaneUpgradeStatus,
    observed: &Observed,
    timeout: Duration,
    component: DataplaneComponentKind,
) -> Step {
    let at_target =
        observed.versions.get(&component).map(String::as_str) == Some(spec.target_version.as_str());
    let ready = observed.ready.get(&component).copied().unwrap_or(false);
    let started_at = entry_for(status, component).and_then(|entry| entry.started_at.as_deref());

    if at_target && ready {
        Step::MarkUpgraded(component)
    } else if expired(started_at, observed.now, timeout) {
        abort(status, Failure::ReadyTimeout(component))
    } else if !at_target {
        Step::Patch(component)
    } else {
        Step::Wait
    }
}

fn abort(status: &IdentityDataplaneUpgradeStatus, cause: Failure) -> Step {
    let restore = restorable(status);
    if restore.is_empty() {
        Step::Fail(cause)
    } else {
        Step::RollBack { cause, restore }
    }
}

fn restorable(status: &IdentityDataplaneUpgradeStatus) -> Vec<(DataplaneComponentKind, String)> {
    status
        .components
        .iter()
        .rev()
        .filter(|entry| {
            matches!(
                entry.state,
                ComponentUpgradeState::Upgrading | ComponentUpgradeState::Upgraded
            )
        })
        .filter_map(|entry| {
            let kind = DataplaneComponentKind::from_component(entry.name)?;
            Some((kind, entry.previous_version.clone()?))
        })
        .collect()
}

fn expired(started_at: Option<&str>, now: DateTime<Utc>, timeout: Duration) -> bool {
    let Some(started) = started_at.and_then(|at| DateTime::parse_from_rfc3339(at).ok()) else {
        return true;
    };
    now.signed_duration_since(started)
        .to_std()
        .is_ok_and(|elapsed| elapsed >= timeout)
}

pub fn is_terminal(phase: DataplaneUpgradePhase) -> bool {
    matches!(
        phase,
        DataplaneUpgradePhase::Completed
            | DataplaneUpgradePhase::Failed
            | DataplaneUpgradePhase::RolledBack
    )
}

fn entry_for(
    status: &IdentityDataplaneUpgradeStatus,
    component: DataplaneComponentKind,
) -> Option<&ComponentUpgradeStatus> {
    let name = DataplaneComponent::from(component);
    status.components.iter().find(|entry| entry.name == name)
}

fn timestamp(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn advance(
    spec: &IdentityDataplaneUpgradeSpec,
    status: &IdentityDataplaneUpgradeStatus,
    step: &Step,
    now: DateTime<Utc>,
) -> IdentityDataplaneUpgradeStatus {
    let mut next = status.clone();
    match step {
        Step::Idle | Step::Wait | Step::Patch(_) => {}
        Step::Start {
            component,
            previous,
        } => {
            next.phase = DataplaneUpgradePhase::Upgrading;
            let record = ComponentUpgradeStatus {
                name: DataplaneComponent::from(*component),
                previous_version: Some(previous.clone()),
                state: ComponentUpgradeState::Upgrading,
                started_at: Some(timestamp(now)),
            };
            match next
                .components
                .iter_mut()
                .find(|entry| entry.name == record.name)
            {
                Some(entry) => *entry = record,
                None => next.components.push(record),
            }
        }
        Step::MarkUpgraded(component) => {
            let name = DataplaneComponent::from(*component);
            for entry in next
                .components
                .iter_mut()
                .filter(|entry| entry.name == name)
            {
                entry.state = ComponentUpgradeState::Upgraded;
            }
        }
        Step::Complete => {
            next.phase = DataplaneUpgradePhase::Completed;
            next.current_version = Some(spec.target_version.clone());
            next.conditions = vec![condition(
                SUCCEEDED,
                ConditionStatus::True,
                "UpgradeCompleted",
                format!("every requested component runs {}", spec.target_version),
                now,
            )];
        }
        Step::Fail(cause) => {
            next.phase = DataplaneUpgradePhase::Failed;
            next.conditions = vec![failure_condition(cause, cause.message(), now)];
        }
        Step::RollBack { cause, .. } => {
            next.phase = DataplaneUpgradePhase::RolledBack;
            for entry in next.components.iter_mut().filter(|entry| {
                matches!(
                    entry.state,
                    ComponentUpgradeState::Upgrading | ComponentUpgradeState::Upgraded
                )
            }) {
                entry.state = ComponentUpgradeState::RolledBack;
            }
            next.conditions = vec![failure_condition(
                cause,
                format!("rolled back: {}", cause.message()),
                now,
            )];
        }
    }
    next.progress = Some(progress(spec, &next));
    next
}

fn failure_condition(cause: &Failure, message: String, now: DateTime<Utc>) -> Condition {
    condition(
        cause.condition_type(),
        ConditionStatus::False,
        cause.reason(),
        message,
        now,
    )
}

fn condition(
    condition_type: &str,
    status: ConditionStatus,
    reason: &str,
    message: String,
    now: DateTime<Utc>,
) -> Condition {
    Condition {
        condition_type: condition_type.to_string(),
        status,
        last_transition_time: timestamp(now),
        reason: Some(reason.to_string()),
        message: Some(message),
    }
}

fn progress(
    spec: &IdentityDataplaneUpgradeSpec,
    status: &IdentityDataplaneUpgradeStatus,
) -> String {
    let requested = DataplaneComponentKind::expand(&spec.components);
    let upgraded = requested
        .iter()
        .filter(|component| {
            entry_for(status, **component)
                .is_some_and(|entry| entry.state == ComponentUpgradeState::Upgraded)
        })
        .count();
    format!("{upgraded}/{} components updated", requested.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    use DataplaneComponentKind::{Genesis, Herald, Operator};

    const TARGET: &str = "dpu-5";
    const OLD: &str = "dpu-4";
    const TIMEOUT: Duration = Duration::from_secs(300);

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_800_000_000 + seconds, 0).unwrap()
    }

    fn spec(components: Vec<DataplaneComponent>) -> IdentityDataplaneUpgradeSpec {
        IdentityDataplaneUpgradeSpec {
            dataplane_id: "dp".to_string(),
            target_version: TARGET.to_string(),
            components,
            strategy: DataplaneUpgradeStrategy::Rolling,
            max_unavailable: 1,
        }
    }

    fn all() -> IdentityDataplaneUpgradeSpec {
        spec(vec![DataplaneComponent::All])
    }

    fn record(
        component: DataplaneComponentKind,
        state: ComponentUpgradeState,
        started: i64,
    ) -> ComponentUpgradeStatus {
        ComponentUpgradeStatus {
            name: component.into(),
            previous_version: Some(OLD.to_string()),
            state,
            started_at: Some(timestamp(at(started))),
        }
    }

    fn status(components: Vec<ComponentUpgradeStatus>) -> IdentityDataplaneUpgradeStatus {
        IdentityDataplaneUpgradeStatus {
            phase: DataplaneUpgradePhase::Upgrading,
            components,
            ..Default::default()
        }
    }

    fn observed(
        versions: &[(DataplaneComponentKind, &str)],
        ready: &[(DataplaneComponentKind, bool)],
        now: i64,
    ) -> Observed {
        Observed {
            versions: versions
                .iter()
                .map(|(component, version)| (*component, version.to_string()))
                .collect(),
            ready: ready.iter().copied().collect(),
            now: at(now),
        }
    }

    fn old_everywhere() -> Vec<(DataplaneComponentKind, &'static str)> {
        vec![(Herald, OLD), (Genesis, OLD), (Operator, OLD)]
    }

    #[test]
    fn terminal_phases_are_idle_whatever_the_spec_says() {
        for phase in [
            DataplaneUpgradePhase::Completed,
            DataplaneUpgradePhase::Failed,
            DataplaneUpgradePhase::RolledBack,
        ] {
            let status = IdentityDataplaneUpgradeStatus {
                phase,
                ..Default::default()
            };
            let mut canary = all();
            canary.strategy = DataplaneUpgradeStrategy::Canary;

            assert_eq!(preflight(&canary, &status), Some(Step::Idle));
        }
    }

    #[test]
    fn canary_fails_before_anything_is_observed() {
        let mut canary = all();
        canary.strategy = DataplaneUpgradeStrategy::Canary;

        assert_eq!(
            preflight(&canary, &IdentityDataplaneUpgradeStatus::default()),
            Some(Step::Fail(Failure::CanaryNotSupported))
        );
    }

    #[test]
    fn empty_component_list_fails() {
        assert_eq!(
            preflight(
                &spec(Vec::new()),
                &IdentityDataplaneUpgradeStatus::default()
            ),
            Some(Step::Fail(Failure::NoComponents))
        );
    }

    #[test]
    fn rolling_pending_upgrade_passes_preflight() {
        assert_eq!(
            preflight(&all(), &IdentityDataplaneUpgradeStatus::default()),
            None
        );
    }

    #[test]
    fn first_step_starts_herald_with_its_observed_version() {
        let step = decide(
            &all(),
            &IdentityDataplaneUpgradeStatus::default(),
            &observed(&old_everywhere(), &[], 0),
            TIMEOUT,
        );

        assert_eq!(
            step,
            Step::Start {
                component: Herald,
                previous: OLD.to_string()
            }
        );
    }

    #[test]
    fn components_start_in_upgrade_order_operator_last() {
        let versions = old_everywhere();

        let after_herald = status(vec![record(Herald, ComponentUpgradeState::Upgraded, 0)]);
        assert_eq!(
            decide(
                &all(),
                &after_herald,
                &observed(&versions, &[], 10),
                TIMEOUT
            ),
            Step::Start {
                component: Genesis,
                previous: OLD.to_string()
            }
        );

        let after_genesis = status(vec![
            record(Herald, ComponentUpgradeState::Upgraded, 0),
            record(Genesis, ComponentUpgradeState::Upgraded, 10),
        ]);
        assert_eq!(
            decide(
                &all(),
                &after_genesis,
                &observed(&versions, &[], 20),
                TIMEOUT
            ),
            Step::Start {
                component: Operator,
                previous: OLD.to_string()
            }
        );
    }

    #[test]
    fn only_requested_components_are_visited() {
        let only_operator = spec(vec![DataplaneComponent::Operator]);

        assert_eq!(
            decide(
                &only_operator,
                &IdentityDataplaneUpgradeStatus::default(),
                &observed(&old_everywhere(), &[], 0),
                TIMEOUT
            ),
            Step::Start {
                component: Operator,
                previous: OLD.to_string()
            }
        );
    }

    #[test]
    fn upgrading_component_not_yet_patched_is_patched_again() {
        let status = status(vec![record(Herald, ComponentUpgradeState::Upgrading, 0)]);

        assert_eq!(
            decide(
                &all(),
                &status,
                &observed(&old_everywhere(), &[(Herald, true)], 5),
                TIMEOUT
            ),
            Step::Patch(Herald)
        );
    }

    #[test]
    fn upgrading_component_on_target_but_not_ready_waits() {
        let status = status(vec![record(Herald, ComponentUpgradeState::Upgrading, 0)]);

        assert_eq!(
            decide(
                &all(),
                &status,
                &observed(&[(Herald, TARGET)], &[(Herald, false)], 5),
                TIMEOUT
            ),
            Step::Wait
        );
    }

    #[test]
    fn upgrading_component_on_target_and_ready_is_marked_upgraded() {
        let status = status(vec![record(Herald, ComponentUpgradeState::Upgrading, 0)]);

        assert_eq!(
            decide(
                &all(),
                &status,
                &observed(&[(Herald, TARGET)], &[(Herald, true)], 5),
                TIMEOUT
            ),
            Step::MarkUpgraded(Herald)
        );
    }

    #[test]
    fn ready_on_the_old_version_is_not_progress() {
        let status = status(vec![record(Herald, ComponentUpgradeState::Upgrading, 0)]);

        assert_eq!(
            decide(
                &all(),
                &status,
                &observed(&[(Herald, OLD)], &[(Herald, true)], 5),
                TIMEOUT
            ),
            Step::Patch(Herald)
        );
    }

    #[test]
    fn restarted_operator_already_on_target_and_ready_finishes_the_upgrade() {
        let status = status(vec![
            record(Herald, ComponentUpgradeState::Upgraded, 0),
            record(Genesis, ComponentUpgradeState::Upgraded, 10),
            record(Operator, ComponentUpgradeState::Upgrading, 20),
        ]);
        let observed = observed(
            &[(Herald, TARGET), (Genesis, TARGET), (Operator, TARGET)],
            &[(Herald, true), (Genesis, true), (Operator, true)],
            30,
        );

        assert_eq!(
            decide(&all(), &status, &observed, TIMEOUT),
            Step::MarkUpgraded(Operator)
        );

        let done = status_after(&status, Step::MarkUpgraded(Operator));
        assert_eq!(decide(&all(), &done, &observed, TIMEOUT), Step::Complete);
    }

    fn status_after(
        status: &IdentityDataplaneUpgradeStatus,
        step: Step,
    ) -> IdentityDataplaneUpgradeStatus {
        advance(&all(), status, &step, at(0))
    }

    #[test]
    fn resume_does_not_repeat_finished_components() {
        let status = status(vec![
            record(Herald, ComponentUpgradeState::Upgraded, 0),
            record(Genesis, ComponentUpgradeState::Upgrading, 10),
        ]);
        let observed = observed(
            &[(Herald, TARGET), (Genesis, TARGET), (Operator, OLD)],
            &[(Herald, true), (Genesis, false), (Operator, true)],
            15,
        );

        assert_eq!(decide(&all(), &status, &observed, TIMEOUT), Step::Wait);
    }

    #[test]
    fn timeout_rolls_back_in_reverse_order_from_recorded_versions() {
        let status = status(vec![
            record(Herald, ComponentUpgradeState::Upgraded, 0),
            record(Genesis, ComponentUpgradeState::Upgrading, 10),
        ]);
        let observed = observed(
            &[(Herald, TARGET), (Genesis, TARGET), (Operator, OLD)],
            &[(Herald, true), (Genesis, false)],
            310,
        );

        assert_eq!(
            decide(&all(), &status, &observed, TIMEOUT),
            Step::RollBack {
                cause: Failure::ReadyTimeout(Genesis),
                restore: vec![(Genesis, OLD.to_string()), (Herald, OLD.to_string())],
            }
        );
    }

    #[test]
    fn just_under_the_timeout_still_waits() {
        let status = status(vec![record(Herald, ComponentUpgradeState::Upgrading, 0)]);

        assert_eq!(
            decide(
                &all(),
                &status,
                &observed(&[(Herald, TARGET)], &[(Herald, false)], 299),
                TIMEOUT
            ),
            Step::Wait
        );
        assert!(matches!(
            decide(
                &all(),
                &status,
                &observed(&[(Herald, TARGET)], &[(Herald, false)], 300),
                TIMEOUT
            ),
            Step::RollBack { .. }
        ));
    }

    #[test]
    fn readiness_at_the_deadline_wins_over_the_timeout() {
        let status = status(vec![record(Herald, ComponentUpgradeState::Upgrading, 0)]);

        assert_eq!(
            decide(
                &all(),
                &status,
                &observed(&[(Herald, TARGET)], &[(Herald, true)], 900),
                TIMEOUT
            ),
            Step::MarkUpgraded(Herald)
        );
    }

    #[test]
    fn unparsable_start_time_counts_as_expired() {
        let mut status = status(vec![record(Herald, ComponentUpgradeState::Upgrading, 0)]);
        status.components[0].started_at = Some("yesterday".to_string());

        assert!(matches!(
            decide(
                &all(),
                &status,
                &observed(&[(Herald, TARGET)], &[(Herald, false)], 1),
                TIMEOUT
            ),
            Step::RollBack { .. }
        ));
    }

    #[test]
    fn clock_behind_the_recorded_start_does_not_expire() {
        let status = status(vec![record(Herald, ComponentUpgradeState::Upgrading, 100)]);

        assert_eq!(
            decide(
                &all(),
                &status,
                &observed(&[(Herald, TARGET)], &[(Herald, false)], 0),
                TIMEOUT
            ),
            Step::Wait
        );
    }

    #[test]
    fn missing_first_component_fails_without_rollback() {
        let step = decide(
            &all(),
            &IdentityDataplaneUpgradeStatus::default(),
            &observed(&[(Genesis, OLD)], &[], 0),
            TIMEOUT,
        );

        assert_eq!(step, Step::Fail(Failure::ComponentMissing(Herald)));
    }

    #[test]
    fn missing_later_component_rolls_back_finished_ones() {
        let status = status(vec![record(Herald, ComponentUpgradeState::Upgraded, 0)]);
        let step = decide(
            &all(),
            &status,
            &observed(&[(Herald, TARGET)], &[], 10),
            TIMEOUT,
        );

        assert_eq!(
            step,
            Step::RollBack {
                cause: Failure::ComponentMissing(Genesis),
                restore: vec![(Herald, OLD.to_string())],
            }
        );
    }

    #[test]
    fn everything_upgraded_completes() {
        let status = status(vec![
            record(Herald, ComponentUpgradeState::Upgraded, 0),
            record(Genesis, ComponentUpgradeState::Upgraded, 10),
            record(Operator, ComponentUpgradeState::Upgraded, 20),
        ]);

        assert_eq!(
            decide(&all(), &status, &observed(&[], &[], 30), TIMEOUT),
            Step::Complete
        );
    }

    #[test]
    fn start_records_previous_version_and_start_time_before_patching() {
        let next = advance(
            &all(),
            &IdentityDataplaneUpgradeStatus::default(),
            &Step::Start {
                component: Herald,
                previous: OLD.to_string(),
            },
            at(7),
        );

        assert_eq!(next.phase, DataplaneUpgradePhase::Upgrading);
        assert_eq!(next.progress.as_deref(), Some("0/3 components updated"));
        assert_eq!(next.current_version, None);
        assert_eq!(
            next.components,
            vec![record(Herald, ComponentUpgradeState::Upgrading, 7)]
        );
    }

    #[test]
    fn restarting_a_component_replaces_its_record() {
        let mut stale = status(vec![record(Herald, ComponentUpgradeState::Pending, 0)]);
        stale.components[0].previous_version = None;

        let next = advance(
            &all(),
            &stale,
            &Step::Start {
                component: Herald,
                previous: OLD.to_string(),
            },
            at(7),
        );

        assert_eq!(next.components.len(), 1);
        assert_eq!(next.components[0].state, ComponentUpgradeState::Upgrading);
    }

    #[test]
    fn marking_upgraded_updates_progress() {
        let before = status(vec![
            record(Herald, ComponentUpgradeState::Upgraded, 0),
            record(Genesis, ComponentUpgradeState::Upgrading, 10),
        ]);

        let next = advance(&all(), &before, &Step::MarkUpgraded(Genesis), at(20));

        assert_eq!(next.progress.as_deref(), Some("2/3 components updated"));
        assert_eq!(next.phase, DataplaneUpgradePhase::Upgrading);
        assert_eq!(next.current_version, None);
    }

    #[test]
    fn completion_sets_current_version_to_the_target() {
        let next = advance(
            &all(),
            &status(vec![
                record(Herald, ComponentUpgradeState::Upgraded, 0),
                record(Genesis, ComponentUpgradeState::Upgraded, 10),
                record(Operator, ComponentUpgradeState::Upgraded, 20),
            ]),
            &Step::Complete,
            at(30),
        );

        assert_eq!(next.phase, DataplaneUpgradePhase::Completed);
        assert_eq!(next.current_version.as_deref(), Some(TARGET));
        assert_eq!(next.progress.as_deref(), Some("3/3 components updated"));
        assert_eq!(next.conditions[0].condition_type, SUCCEEDED);
        assert_eq!(next.conditions[0].status, ConditionStatus::True);
    }

    #[test]
    fn rollback_marks_touched_components_and_keeps_current_version_unset() {
        let before = status(vec![
            record(Herald, ComponentUpgradeState::Upgraded, 0),
            record(Genesis, ComponentUpgradeState::Upgrading, 10),
        ]);
        let step = decide(
            &all(),
            &before,
            &observed(&[(Herald, TARGET), (Genesis, TARGET)], &[], 400),
            TIMEOUT,
        );

        let next = advance(&all(), &before, &step, at(400));

        assert_eq!(next.phase, DataplaneUpgradePhase::RolledBack);
        assert_eq!(next.current_version, None);
        assert_eq!(next.progress.as_deref(), Some("0/3 components updated"));
        assert!(
            next.components
                .iter()
                .all(|entry| entry.state == ComponentUpgradeState::RolledBack)
        );
        assert_eq!(next.conditions[0].reason.as_deref(), Some("ReadyTimeout"));
        assert_eq!(next.conditions[0].status, ConditionStatus::False);
    }

    #[test]
    fn canary_failure_sets_accepted_false_with_its_reason() {
        let next = advance(
            &all(),
            &IdentityDataplaneUpgradeStatus::default(),
            &Step::Fail(Failure::CanaryNotSupported),
            at(0),
        );

        assert_eq!(next.phase, DataplaneUpgradePhase::Failed);
        assert_eq!(next.conditions[0].condition_type, ACCEPTED);
        assert_eq!(next.conditions[0].status, ConditionStatus::False);
        assert_eq!(
            next.conditions[0].reason.as_deref(),
            Some("CanaryNotSupported")
        );
        assert_eq!(next.current_version, None);
    }

    #[test]
    fn rollback_failure_is_a_failed_upgrade_carrying_the_reason() {
        let next = advance(
            &all(),
            &status(vec![record(Herald, ComponentUpgradeState::Upgrading, 0)]),
            &Step::Fail(Failure::RollbackFailed {
                component: Herald,
                message: "boom".to_string(),
            }),
            at(0),
        );

        assert_eq!(next.phase, DataplaneUpgradePhase::Failed);
        assert_eq!(next.conditions[0].reason.as_deref(), Some("RollbackFailed"));
        assert!(
            next.conditions[0]
                .message
                .as_deref()
                .unwrap()
                .contains("boom")
        );
    }

    #[test]
    fn waiting_and_patching_leave_the_recorded_state_alone() {
        let before = status(vec![record(Herald, ComponentUpgradeState::Upgrading, 0)]);

        for step in [Step::Wait, Step::Patch(Herald), Step::Idle] {
            let next = advance(&all(), &before, &step, at(50));
            assert_eq!(next.components, before.components);
            assert_eq!(next.phase, before.phase);
        }
    }

    #[test]
    fn progress_counts_only_requested_components() {
        let only_two = spec(vec![
            DataplaneComponent::Herald,
            DataplaneComponent::Genesis,
        ]);

        let next = advance(
            &only_two,
            &status(vec![record(Herald, ComponentUpgradeState::Upgrading, 0)]),
            &Step::MarkUpgraded(Herald),
            at(1),
        );

        assert_eq!(next.progress.as_deref(), Some("1/2 components updated"));
    }
}
