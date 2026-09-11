use chrono::{DateTime, Duration, Utc};

use crate::{
    CoreError,
    deployments::DeploymentId,
    metrics::{MetricBucket, MetricKind, TimeRange},
    organisation::OrganisationId,
};

/// What Herald sends for one bucket. Building this always floors `at` to its
/// minute, so a caller cannot accidentally report a bucket the storage layer
/// would then have to reject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordMetricBucketCommand {
    pub deployment_id: DeploymentId,
    pub metric: MetricKind,
    pub bucket: MetricBucket,
    pub value: u64,
}

impl RecordMetricBucketCommand {
    pub fn new(
        deployment_id: DeploymentId,
        metric: MetricKind,
        at: DateTime<Utc>,
        value: u64,
    ) -> Self {
        Self {
            deployment_id,
            metric,
            bucket: MetricBucket::containing(at),
            value,
        }
    }
}

/// What the console asks for: one deployment's series for one metric, over a
/// range.
///
/// `from`/`until` stay as the two instants a caller supplied rather than a
/// [`TimeRange`] built eagerly, so a query assembled from request parameters
/// can fail where the handler turns it into a 400 -- when [`Self::range`] is
/// finally called -- instead of the constructor deciding that on the
/// caller's behalf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeploymentUsageQuery {
    pub organisation_id: OrganisationId,
    pub deployment_id: DeploymentId,
    pub metric: MetricKind,
    from: DateTime<Utc>,
    until: DateTime<Utc>,
}

impl DeploymentUsageQuery {
    pub fn new(
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
        metric: MetricKind,
        from: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Self {
        Self {
            organisation_id,
            deployment_id,
            metric,
            from,
            until,
        }
    }

    pub fn range(&self) -> Result<TimeRange, CoreError> {
        TimeRange::new(self.from, self.until)
    }
}

/// What the console asks for when it wants a single active-user count rather
/// than a series.
///
/// `at` travels with the query instead of being read from the clock inside
/// the service: the counting rule in [`crate::metrics::active_users`] takes
/// time as a parameter on purpose, and a query built from `Utc::now()` at the
/// one edge that is allowed to call it is how that stays true end to end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveUsersQuery {
    pub organisation_id: OrganisationId,
    pub deployment_id: DeploymentId,
    pub at: DateTime<Utc>,
    pub window: Duration,
}

impl ActiveUsersQuery {
    pub fn new(
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
        at: DateTime<Utc>,
        window: Duration,
    ) -> Self {
        Self {
            organisation_id,
            deployment_id,
            at,
            window,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn at() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-01-01T00:00:45Z")
            .expect("a valid instant")
            .with_timezone(&Utc)
    }

    #[test]
    fn recording_a_bucket_floors_the_reported_instant() {
        let command = RecordMetricBucketCommand::new(
            DeploymentId(Uuid::new_v4()),
            MetricKind::Logins,
            at(),
            3,
        );

        assert_eq!(command.bucket, MetricBucket::containing(at()));
        assert_eq!(
            command.bucket.start().to_rfc3339(),
            "2026-01-01T00:00:00+00:00"
        );
    }

    #[test]
    fn a_usage_query_defers_the_range_check_to_range() {
        let query = DeploymentUsageQuery::new(
            OrganisationId(Uuid::new_v4()),
            DeploymentId(Uuid::new_v4()),
            MetricKind::Requests,
            at(),
            at() - Duration::minutes(1),
        );

        assert!(query.range().is_err(), "a reversed range is caught here");
    }

    #[test]
    fn a_usage_query_with_a_sane_range_produces_one() {
        let query = DeploymentUsageQuery::new(
            OrganisationId(Uuid::new_v4()),
            DeploymentId(Uuid::new_v4()),
            MetricKind::Requests,
            at() - Duration::hours(1),
            at(),
        );

        let range = query.range().expect("from before until");
        assert_eq!(range.from(), at() - Duration::hours(1));
        assert_eq!(range.until(), at());
    }

    #[test]
    fn an_active_users_query_carries_the_window_and_the_instant_unchanged() {
        let query = ActiveUsersQuery::new(
            OrganisationId(Uuid::new_v4()),
            DeploymentId(Uuid::new_v4()),
            at(),
            Duration::minutes(30),
        );

        assert_eq!(query.at, at());
        assert_eq!(query.window, Duration::minutes(30));
    }
}
