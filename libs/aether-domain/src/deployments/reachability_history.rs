use std::future::Future;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{CoreError, deployments::DeploymentId};

pub mod service;

/// A single deployment reachability check result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ReachabilityCheck {
    pub deployment_id: DeploymentId,
    pub checked_at: DateTime<Utc>,
    pub reachable: bool,
}

/// Availability over a time window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct UptimeWindow {
    /// Share of the observed time the deployment answered, in percent (0-100),
    /// at full precision. `None` when nothing was observed in the window.
    pub uptime_percent: Option<f64>,
    /// True when the first check of the window is no later than the window
    /// start plus five probe intervals and the last check is no earlier than
    /// `now` minus five probe intervals. Gaps between checks are not part of
    /// this rule; they are simply left out of the percentage.
    pub covers_full_window: bool,
}

/// Uptime metrics for a deployment across multiple time windows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct DeploymentUptime {
    pub deployment_id: DeploymentId,
    /// Uptime over the last 24 hours.
    pub uptime_24h: UptimeWindow,
    /// Uptime over the last 7 days.
    pub uptime_7d: UptimeWindow,
    /// Uptime over the last 30 days.
    pub uptime_30d: UptimeWindow,
}

/// A maximal run of consecutive failed reachability checks.
///
/// `started_at` is the time of the first failed check and `ended_at` the time
/// of the first successful check after it. The gap between the last good check
/// and the first failed one is not counted as downtime, so an interval never
/// overstates the outage; it can understate it by up to one probe tick.
/// `ended_at` is `None` while the deployment is still failing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DowntimeInterval {
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_seconds: Option<i64>,
}

/// Downtime intervals of a deployment over a window, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DeploymentDowntime {
    pub deployment_id: DeploymentId,
    pub intervals: Vec<DowntimeInterval>,
}

/// Derives downtime intervals from checks sorted by `checked_at` ascending.
pub fn downtime_intervals(checks: &[ReachabilityCheck]) -> Vec<DowntimeInterval> {
    let mut intervals = Vec::new();
    let mut open: Option<DateTime<Utc>> = None;

    for check in checks {
        match (open, check.reachable) {
            (None, false) => open = Some(check.checked_at),
            (Some(started_at), true) => {
                intervals.push(DowntimeInterval {
                    started_at,
                    ended_at: Some(check.checked_at),
                    duration_seconds: Some((check.checked_at - started_at).num_seconds()),
                });
                open = None;
            }
            _ => {}
        }
    }

    if let Some(started_at) = open {
        intervals.push(DowntimeInterval {
            started_at,
            ended_at: None,
            duration_seconds: None,
        });
    }

    intervals
}

/// Longest time a check is trusted to still describe the deployment: five
/// times the 60 s probe interval. Time between two checks further apart than
/// this is unobserved, so a control plane outage counts neither as uptime nor
/// as downtime.
pub const MAX_GAP: Duration = Duration::seconds(300);

/// Availability over `window` ending at `now`, from checks sorted by
/// `checked_at` ascending.
///
/// A check carries its state until the next check, or until `now` for the
/// last one. Only checks in `[now - window, now]` are used, and the time from
/// the window start to the first of them is unobserved.
pub fn availability(
    checks: &[ReachabilityCheck],
    now: DateTime<Utc>,
    window: Duration,
) -> UptimeWindow {
    let start = now - window;
    let inside: Vec<&ReachabilityCheck> = checks
        .iter()
        .filter(|check| check.checked_at >= start && check.checked_at <= now)
        .collect();

    let mut observed = 0_i64;
    let mut down = 0_i64;
    let mut account = |from: DateTime<Utc>, to: DateTime<Utc>, reachable: bool| {
        let gap = to - from;
        if gap <= MAX_GAP {
            let millis = gap.num_milliseconds();
            observed += millis;
            if !reachable {
                down += millis;
            }
        }
    };

    for pair in inside.windows(2) {
        account(pair[0].checked_at, pair[1].checked_at, pair[0].reachable);
    }
    if let Some(last) = inside.last() {
        account(last.checked_at, now, last.reachable);
    }

    let covers_full_window = match (inside.first(), inside.last()) {
        (Some(first), Some(last)) => {
            first.checked_at <= start + MAX_GAP && last.checked_at >= now - MAX_GAP
        }
        _ => false,
    };

    UptimeWindow {
        uptime_percent: (observed > 0).then(|| 100.0 * (1.0 - down as f64 / observed as f64)),
        covers_full_window,
    }
}

#[cfg_attr(test, mockall::automock)]
pub trait ReachabilityCheckRepository: Send + Sync {
    fn record_check(
        &self,
        check: ReachabilityCheck,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn get_checks_since(
        &self,
        deployment_id: DeploymentId,
        since: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<ReachabilityCheck>, CoreError>> + Send;

    fn purge_old_checks(
        &self,
        retention: chrono::Duration,
    ) -> impl Future<Output = Result<u64, CoreError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000 + seconds, 0).unwrap()
    }

    fn check(seconds: i64, reachable: bool) -> ReachabilityCheck {
        ReachabilityCheck {
            deployment_id: DeploymentId(uuid::Uuid::nil()),
            checked_at: at(seconds),
            reachable,
        }
    }

    #[test]
    fn no_failure_no_interval() {
        let checks = [check(0, true), check(30, true)];
        assert!(downtime_intervals(&checks).is_empty());
        assert!(downtime_intervals(&[]).is_empty());
    }

    #[test]
    fn one_closed_interval() {
        let checks = [
            check(0, true),
            check(30, false),
            check(60, false),
            check(90, true),
        ];
        assert_eq!(
            downtime_intervals(&checks),
            vec![DowntimeInterval {
                started_at: at(30),
                ended_at: Some(at(90)),
                duration_seconds: Some(60),
            }]
        );
    }

    #[test]
    fn still_failing_is_open() {
        let checks = [check(0, true), check(30, false), check(60, false)];
        assert_eq!(
            downtime_intervals(&checks),
            vec![DowntimeInterval {
                started_at: at(30),
                ended_at: None,
                duration_seconds: None,
            }]
        );
    }

    #[test]
    fn two_intervals() {
        let checks = [
            check(0, false),
            check(30, true),
            check(60, true),
            check(90, false),
            check(120, true),
        ];
        let intervals = downtime_intervals(&checks);
        assert_eq!(intervals.len(), 2);
        assert_eq!(intervals[0].started_at, at(0));
        assert_eq!(intervals[0].duration_seconds, Some(30));
        assert_eq!(intervals[1].started_at, at(90));
        assert_eq!(intervals[1].ended_at, Some(at(120)));
    }

    #[test]
    fn single_failed_check_between_good_ones() {
        let checks = [check(0, true), check(30, false), check(60, true)];
        let intervals = downtime_intervals(&checks);
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals[0].duration_seconds, Some(30));
    }

    fn series(
        from: i64,
        to: i64,
        step: i64,
        reachable: impl Fn(i64) -> bool,
    ) -> Vec<ReachabilityCheck> {
        (from..=to)
            .step_by(step as usize)
            .map(|t| check(t, reachable(t)))
            .collect()
    }

    const DAY: i64 = 86_400;

    fn percent(checks: &[ReachabilityCheck], now: i64, window: i64) -> Option<f64> {
        availability(checks, at(now), Duration::seconds(window)).uptime_percent
    }

    #[test]
    fn continuous_up_is_one_hundred() {
        let checks = series(0, DAY, 60, |_| true);
        let window = availability(&checks, at(DAY), Duration::seconds(DAY));
        assert_eq!(window.uptime_percent, Some(100.0));
        assert!(window.covers_full_window);
    }

    #[test]
    fn four_minutes_down_in_thirty_days() {
        let month = 30 * DAY;
        let checks = series(0, month, 60, |t| !(1_000_000..1_000_240).contains(&t));
        let window = availability(&checks, at(month), Duration::seconds(month));
        let expected = 100.0 * (1.0 - 240.0 / 2_592_000.0);
        assert!((window.uptime_percent.unwrap() - expected).abs() < 1e-9);
        assert!((expected - 99.99074).abs() < 1e-5);
        assert!(window.covers_full_window);
    }

    #[test]
    fn still_failing_at_now_counts_until_now() {
        let checks = [check(0, true), check(60, true), check(120, false)];
        assert_eq!(
            percent(&checks, 180, 180),
            Some(100.0 * (1.0 - 60.0 / 180.0))
        );
    }

    #[test]
    fn a_gap_longer_than_max_gap_is_unobserved() {
        let checks = [
            check(0, true),
            check(60, true),
            check(1_000, false),
            check(1_060, true),
        ];
        assert_eq!(
            percent(&checks, 1_060, 1_060),
            Some(100.0 * (1.0 - 60.0 / 120.0))
        );

        let up_both_sides = [check(0, true), check(60, true), check(1_000, true)];
        assert_eq!(percent(&up_both_sides, 1_000, 1_000), Some(100.0));
    }

    #[test]
    fn a_gap_of_exactly_max_gap_is_observed() {
        let checks = [check(0, false), check(300, true)];
        assert_eq!(percent(&checks, 300, 300), Some(0.0));
    }

    #[test]
    fn a_stale_last_check_leaves_the_tail_unobserved() {
        let checks = [check(0, true), check(60, false)];
        assert_eq!(percent(&checks, 10_000, 10_000), Some(100.0));
        assert!(!availability(&checks, at(10_000), Duration::seconds(10_000)).covers_full_window);
    }

    #[test]
    fn no_check_means_no_percentage() {
        let window = availability(&[], at(100), Duration::seconds(100));
        assert_eq!(window.uptime_percent, None);
        assert!(!window.covers_full_window);
    }

    #[test]
    fn a_single_check_observes_until_now() {
        assert_eq!(percent(&[check(70, true)], 100, 100), Some(100.0));
        assert_eq!(percent(&[check(70, false)], 100, 100), Some(0.0));
        assert_eq!(percent(&[check(100, true)], 100, 100), None);
    }

    #[test]
    fn a_single_old_check_is_not_observed() {
        assert_eq!(percent(&[check(0, true)], 10_000, 10_000), None);
    }

    #[test]
    fn a_failure_before_the_window_start_is_ignored() {
        let checks = [
            check(0, false),
            check(60, false),
            check(120, true),
            check(180, true),
        ];
        let window = availability(&checks, at(240), Duration::seconds(120));
        assert_eq!(window.uptime_percent, Some(100.0));
        assert!(window.covers_full_window);
    }

    #[test]
    fn a_window_longer_than_the_history_is_not_covered() {
        let checks = series(0, 3_600, 60, |_| true);
        let window = availability(&checks, at(3_600), Duration::seconds(DAY));
        assert_eq!(window.uptime_percent, Some(100.0));
        assert!(!window.covers_full_window);
    }

    #[test]
    fn a_long_gap_is_one_interval_but_unobserved_for_availability() {
        let checks = [check(0, false), check(1_000, true)];
        assert_eq!(downtime_intervals(&checks)[0].duration_seconds, Some(1_000));
        assert_eq!(percent(&checks, 1_000, 1_000), None);
    }

    #[test]
    fn uptime_window_serializes() {
        let window = UptimeWindow {
            uptime_percent: Some(99.5),
            covers_full_window: true,
        };
        let json = serde_json::to_string(&window).unwrap();
        assert!(json.contains("99.5"));
        assert!(json.contains("true"));

        let empty = UptimeWindow {
            uptime_percent: None,
            covers_full_window: false,
        };
        assert!(
            serde_json::to_string(&empty)
                .unwrap()
                .contains("\"uptime_percent\":null")
        );
    }
}
