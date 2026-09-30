use std::future::Future;

use chrono::{DateTime, Utc};
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

/// Uptime data for a specific time window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct UptimeWindow {
    /// Percentage of checks that succeeded (0-100).
    pub uptime_percent: f64,
    /// Whether this window covers the full requested duration or less.
    /// If false, the uptime is computed over a shorter observed period.
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

#[cfg_attr(test, mockall::automock)]
pub trait ReachabilityCheckRepository: Send + Sync {
    fn record_check(
        &self,
        check: ReachabilityCheck,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn get_uptime(
        &self,
        deployment_id: DeploymentId,
    ) -> impl Future<Output = Result<DeploymentUptime, CoreError>> + Send;

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

    #[test]
    fn uptime_window_serializes() {
        let window = UptimeWindow {
            uptime_percent: 99.5,
            covers_full_window: true,
        };
        let json = serde_json::to_string(&window).unwrap();
        assert!(json.contains("99.5"));
        assert!(json.contains("true"));
    }

    #[test]
    fn deployment_uptime_serializes() {
        let uptime = DeploymentUptime {
            deployment_id: DeploymentId(uuid::Uuid::nil()),
            uptime_24h: UptimeWindow {
                uptime_percent: 100.0,
                covers_full_window: true,
            },
            uptime_7d: UptimeWindow {
                uptime_percent: 99.9,
                covers_full_window: true,
            },
            uptime_30d: UptimeWindow {
                uptime_percent: 99.5,
                covers_full_window: true,
            },
        };
        let json = serde_json::to_string(&uptime).unwrap();
        assert!(!json.is_empty());
    }
}
