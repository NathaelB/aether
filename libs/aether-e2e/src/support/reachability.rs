use aether_domain::deployments::DeploymentId;
use aether_domain::deployments::reachability_history::ReachabilityCheck;
use chrono::{DateTime, Utc};

pub fn at(seconds: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(1_700_000_000 + seconds, 0).expect("a valid timestamp")
}

pub fn check(seconds: i64, reachable: bool) -> ReachabilityCheck {
    ReachabilityCheck {
        deployment_id: DeploymentId(uuid::Uuid::nil()),
        checked_at: at(seconds),
        reachable,
    }
}

pub fn series(from: i64, to: i64, step: i64, reachable: bool) -> Vec<ReachabilityCheck> {
    (from..=to)
        .step_by(step as usize)
        .map(|seconds| check(seconds, reachable))
        .collect()
}

pub fn mark_down(checks: &mut [ReachabilityCheck], from: i64, to: i64) {
    for check in checks
        .iter_mut()
        .filter(|check| (at(from)..at(to)).contains(&check.checked_at))
    {
        check.reachable = false;
    }
}
