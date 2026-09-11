use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};

use super::deployment::{DeploymentId, DeploymentKind};

/// Seconds in one bucket.
///
/// The control plane floors every reported instant into a one-minute bucket,
/// so a Herald that aggregated over anything else would have its windows
/// silently merged on arrival rather than rejected.
pub const BUCKET_SECONDS: i64 = 60;

/// The one-minute window a value covers, floored on construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UsageBucket(DateTime<Utc>);

impl UsageBucket {
    pub fn containing(at: DateTime<Utc>) -> Self {
        let seconds = at.timestamp();
        let floored = seconds - seconds.rem_euclid(BUCKET_SECONDS);
        Self(
            DateTime::<Utc>::from_timestamp(floored, 0)
                .expect("a valid instant floors to another valid instant"),
        )
    }

    pub fn start(&self) -> DateTime<Utc> {
        self.0
    }

    pub fn next(&self) -> Self {
        Self(self.0 + Duration::seconds(BUCKET_SECONDS))
    }
}

/// What the control plane can be asked "how much" about.
///
/// Closed, and named with the wire spellings the control plane accepts: a
/// metric it does not know is rejected for the whole batch, so an invented
/// name here would cost every other point in the same request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UsageMetric {
    Requests,
    TokenEvents,
    Logins,
    ActiveUsers,
}

impl UsageMetric {
    pub fn wire_name(&self) -> &'static str {
        match self {
            Self::Requests => "requests",
            Self::TokenEvents => "token_events",
            Self::Logins => "logins",
            Self::ActiveUsers => "active_users",
        }
    }
}

impl std::fmt::Display for UsageMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.wire_name())
    }
}

/// One aggregated bucket, ready to be carried upward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsagePoint {
    pub metric: UsageMetric,
    pub bucket: UsageBucket,
    pub value: u64,
}

/// Cumulative totals as one instance reported them at one instant.
///
/// Cumulative, not per-minute: every source Herald can read exposes counters
/// that only ever grow, so the per-minute numbers are differences Herald
/// computes here rather than numbers anyone hands it.
///
/// A metric absent from `totals` was not exposed. That is not a zero, and the
/// aggregator must never turn it into one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterSample {
    pub taken_at: DateTime<Utc>,
    pub totals: BTreeMap<UsageMetric, u64>,
}

impl CounterSample {
    pub fn new(taken_at: DateTime<Utc>) -> Self {
        Self {
            taken_at,
            totals: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with(mut self, metric: UsageMetric, total: u64) -> Self {
        self.totals.insert(metric, total);
        self
    }
}

/// Everything a source needs to find one deployment's IAM instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageTarget {
    pub deployment_id: DeploymentId,
    pub kind: DeploymentKind,
    pub namespace: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(rfc3339: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(rfc3339)
            .expect("a valid instant")
            .with_timezone(&Utc)
    }

    #[test]
    fn a_bucket_floors_to_the_start_of_its_minute() {
        let bucket = UsageBucket::containing(at("2026-01-01T10:05:42Z"));

        assert_eq!(bucket.start(), at("2026-01-01T10:05:00Z"));
    }

    #[test]
    fn the_next_bucket_starts_one_minute_later() {
        let bucket = UsageBucket::containing(at("2026-01-01T10:05:42Z"));

        assert_eq!(bucket.next().start(), at("2026-01-01T10:06:00Z"));
    }

    /// These strings are the control plane's closed set. A spelling that
    /// drifts from it is a 400 for the whole batch, not just for one point.
    #[test]
    fn every_metric_carries_the_name_the_control_plane_accepts() {
        assert_eq!(UsageMetric::Requests.wire_name(), "requests");
        assert_eq!(UsageMetric::TokenEvents.wire_name(), "token_events");
        assert_eq!(UsageMetric::Logins.wire_name(), "logins");
        assert_eq!(UsageMetric::ActiveUsers.wire_name(), "active_users");
    }
}
