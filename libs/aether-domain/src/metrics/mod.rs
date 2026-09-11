use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{CoreError, deployments::DeploymentId};

pub mod commands;
pub mod ports;
pub mod service;

/// What the platform can be asked "how much" about, as a closed set.
///
/// A free string here would let a typo open a second series next to the real
/// one -- `"login"` reported once next to a thousand rows of `"logins"` --
/// and nothing would ever notice, because nothing looks for a series it does
/// not know the name of. Naming the set closes that off at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    Requests,
    TokenEvents,
    Logins,
    /// How many distinct users a deployment has seen active, as of a given
    /// minute. Unlike the other three, this is not a per-minute delta: see
    /// [`active_users`] for what that means for how it is read.
    ActiveUsers,
}

impl std::fmt::Display for MetricKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Requests => write!(f, "requests"),
            Self::TokenEvents => write!(f, "token_events"),
            Self::Logins => write!(f, "logins"),
            Self::ActiveUsers => write!(f, "active_users"),
        }
    }
}

impl TryFrom<&str> for MetricKind {
    type Error = CoreError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "requests" => Ok(Self::Requests),
            "token_events" => Ok(Self::TokenEvents),
            "logins" => Ok(Self::Logins),
            "active_users" => Ok(Self::ActiveUsers),
            other => Err(CoreError::InternalError(format!(
                "unknown metric '{other}'"
            ))),
        }
    }
}

/// The one-minute window a point was reported for.
///
/// Floored on construction rather than trusted from the caller: the storage
/// decision fixed one-minute buckets (roughly 500k rows a month for a hundred
/// instances, which Postgres carries without help), and a stray sub-minute
/// timestamp reported as its own bucket would silently split one minute of
/// usage into several rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, ToSchema)]
pub struct MetricBucket(DateTime<Utc>);

impl MetricBucket {
    pub fn containing(at: DateTime<Utc>) -> Self {
        let seconds = at.timestamp();
        let floored = seconds - seconds.rem_euclid(60);
        Self(
            DateTime::<Utc>::from_timestamp(floored, 0)
                .expect("a valid instant floors to another valid instant"),
        )
    }

    pub fn start(&self) -> DateTime<Utc> {
        self.0
    }
}

/// A value the write side hands the repository. Already valid by
/// construction: the bucket is floored, the metric is one of the closed set,
/// and `u64` rules out a negative count -- there is nothing left for a
/// dedicated constructor to guard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct MetricPoint {
    pub deployment_id: DeploymentId,
    pub metric: MetricKind,
    pub bucket: MetricBucket,
    pub value: u64,
}

/// One bucket the source actually reported, as it comes back from a read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct ReportedBucket {
    pub bucket: MetricBucket,
    pub value: u64,
}

/// A deployment's series for one metric, restricted to a range.
///
/// `points` holds only the buckets the source reported. A minute nothing
/// arrived for is missing from the vector, never present with a value of
/// zero: a deployment nobody used and a data plane that has gone quiet must
/// not look the same, and inventing a zero to fill the gap is what would
/// make them look the same.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct MetricSeries {
    pub deployment_id: DeploymentId,
    pub metric: MetricKind,
    pub points: Vec<ReportedBucket>,
}

/// A span of time to read a series over. Constructed rather than assumed
/// ordered, because both ends usually come straight from a caller's query
/// parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct TimeRange {
    from: DateTime<Utc>,
    until: DateTime<Utc>,
}

impl TimeRange {
    pub fn new(from: DateTime<Utc>, until: DateTime<Utc>) -> Result<Self, CoreError> {
        if from > until {
            return Err(CoreError::InternalError(format!(
                "invalid time range: {from} is after {until}"
            )));
        }

        Ok(Self { from, until })
    }

    pub fn from(&self) -> DateTime<Utc> {
        self.from
    }

    pub fn until(&self) -> DateTime<Utc> {
        self.until
    }
}

/// The one definition of an active user, for every caller that counts one.
///
/// `ActiveUsers` buckets are not a delta the way `Requests` or `Logins` are:
/// each one is the data plane's own count of distinct users it has seen
/// active as of that minute, computed with a view into its cluster this
/// platform never has. Summing several of those buckets would count someone
/// active across three minutes three times over, which answers a different
/// question -- "how much activity" -- and is exactly the kind of divergence
/// issue #123 exists to close off.
///
/// An active user, then, is counted from the single most recently reported
/// bucket that falls inside `(at - window, at]`, taken once rather than
/// added to anything else. If no bucket falls inside that window, the answer
/// is `None`: the deployment has gone quiet, which is a different fact from
/// "reported, and nobody was active", and this function will not blur the
/// two by guessing zero.
///
/// `window` is a parameter rather than a constant so that a caller counting
/// daily active users and one counting active users in the last five minutes
/// both go through this one function, and can never answer differently by
/// accident.
pub fn active_users(points: &[ReportedBucket], at: DateTime<Utc>, window: Duration) -> Option<u64> {
    let earliest = at - window;

    points
        .iter()
        .filter(|point| point.bucket.start() > earliest && point.bucket.start() <= at)
        .max_by_key(|point| point.bucket.start())
        .map(|point| point.value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-01-01T00:00:30Z")
            .expect("a valid instant")
            .with_timezone(&Utc)
    }

    fn bucket(minutes_before: i64) -> MetricBucket {
        MetricBucket::containing(at() - Duration::minutes(minutes_before))
    }

    #[test]
    fn every_metric_kind_round_trips_through_its_wire_name() {
        for metric in [
            MetricKind::Requests,
            MetricKind::TokenEvents,
            MetricKind::Logins,
            MetricKind::ActiveUsers,
        ] {
            let name = metric.to_string();
            assert_eq!(MetricKind::try_from(name.as_str()).expect("known"), metric);
        }
    }

    /// A value the database should not be able to hold, but the check
    /// constraint is the only thing stopping it. Saying so beats guessing a
    /// plausible metric and carrying the wrong series onward.
    #[test]
    fn an_unknown_metric_name_is_reported_rather_than_guessed() {
        assert!(MetricKind::try_from("sign_ins").is_err());
    }

    #[test]
    fn a_bucket_floors_to_the_start_of_its_minute() {
        let floored = MetricBucket::containing(at());

        assert_eq!(floored.start().to_rfc3339(), "2026-01-01T00:00:00+00:00");
    }

    #[test]
    fn a_bucket_that_is_already_floored_is_unchanged() {
        let once = MetricBucket::containing(at());
        let twice = MetricBucket::containing(once.start());

        assert_eq!(once, twice);
    }

    #[test]
    fn a_time_range_with_the_ends_swapped_is_refused() {
        let error = TimeRange::new(at(), at() - Duration::minutes(1))
            .expect_err("until before from is not a range");

        assert!(matches!(error, CoreError::InternalError(_)));
    }

    #[test]
    fn a_time_range_where_both_ends_are_equal_is_a_single_instant() {
        assert!(TimeRange::new(at(), at()).is_ok());
    }

    #[test]
    fn no_bucket_at_all_is_not_zero_active_users() {
        let result = active_users(&[], at(), Duration::minutes(30));
        assert_eq!(result, None);
    }

    /// The fact issue #123 exists to protect: a deployment that reported
    /// activity two hours ago, and nothing since, is not the same as a
    /// deployment with zero active users right now.
    #[test]
    fn a_bucket_older_than_the_window_is_not_zero_active_users() {
        let points = [ReportedBucket {
            bucket: bucket(120),
            value: 7,
        }];

        let result = active_users(&points, at(), Duration::minutes(30));
        assert_eq!(
            result, None,
            "a stale bucket must read as unknown, not as the number it once said"
        );
    }

    #[test]
    fn a_bucket_reporting_zero_is_a_real_zero() {
        let points = [ReportedBucket {
            bucket: bucket(1),
            value: 0,
        }];

        let result = active_users(&points, at(), Duration::minutes(30));
        assert_eq!(result, Some(0));
    }

    /// The falsification case: summing would answer 12, not 5. If this ever
    /// starts summing again, this is the test that catches it.
    #[test]
    fn a_user_active_in_two_buckets_is_counted_once() {
        let points = [
            ReportedBucket {
                bucket: bucket(10),
                value: 5,
            },
            ReportedBucket {
                bucket: bucket(2),
                value: 5,
            },
        ];

        let result = active_users(&points, at(), Duration::minutes(30));
        assert_eq!(result, Some(5), "the most recent bucket wins, nothing sums");
    }

    #[test]
    fn a_wider_window_can_reach_further_back_without_changing_the_function() {
        let points = [ReportedBucket {
            bucket: bucket(120),
            value: 9,
        }];

        assert_eq!(active_users(&points, at(), Duration::minutes(30)), None);
        assert_eq!(
            active_users(&points, at(), Duration::hours(3)),
            Some(9),
            "the same points and the same function, only the window changed"
        );
    }
}
