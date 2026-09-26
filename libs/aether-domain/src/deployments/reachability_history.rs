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

    fn purge_old_checks(
        &self,
        retention: chrono::Duration,
    ) -> impl Future<Output = Result<u64, CoreError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

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
