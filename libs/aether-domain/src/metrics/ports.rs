use std::future::Future;

use aether_auth::Identity;

use crate::{
    CoreError,
    deployments::DeploymentId,
    metrics::{
        MetricKind, MetricPoint, MetricSeries, TimeRange,
        commands::{ActiveUsersQuery, DeploymentUsageQuery, RecordMetricBucketCommand},
    },
    organisation::OrganisationId,
};

/// Where Herald's reports land.
///
/// Kept apart from [`MetricsReadRepository`] rather than folded into one
/// trait: writing is driven by every data plane's report cycle and reading by
/// whatever the console asks for, the two do not grow at the same rate, and a
/// single trait would force every implementation -- including a test double
/// that only ever needs one side -- to answer questions it has no business
/// answering.
#[cfg_attr(test, mockall::automock)]
pub trait MetricsWriteRepository: Send + Sync {
    /// Idempotent on `(deployment, metric, bucket)`.
    ///
    /// Herald resends the buckets in its window after a restart, and the
    /// retried report may be the more complete one -- a bucket that was only
    /// half observed before a crash, filled in on replay. A second write for
    /// the same three therefore overwrites the value rather than adding to
    /// it or being dropped: summing would double a number nobody doubled,
    /// and discarding it would keep whatever partial figure the interruption
    /// left behind.
    fn record_bucket(
        &self,
        point: MetricPoint,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;
}

/// What the console reads.
#[cfg_attr(test, mockall::automock)]
pub trait MetricsReadRepository: Send + Sync {
    /// One deployment's series for one metric, restricted to `range`, oldest
    /// first.
    ///
    /// Scoped to `organisation_id` in the query itself rather than checked
    /// afterwards: a `deployment_id` that belongs to a different organisation
    /// reads back with no points, exactly like one that belongs to this
    /// organisation and simply has no data yet. An org boundary that
    /// answered those two cases differently would tell a caller something it
    /// has no business learning -- that the id exists at all, just somewhere
    /// else.
    fn series_for_deployment(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
        metric: MetricKind,
        range: TimeRange,
    ) -> impl Future<Output = Result<MetricSeries, CoreError>> + Send;
}

/// Recording is un-gated by design, the same reasoning the audit trail's own
/// `record` uses: Herald's report reaches this service already
/// authenticated as the data plane's own identity at the API boundary, and a
/// permission check here would just be a second, divergent copy of a
/// decision already made once.
///
/// Reading is scoped to one organisation's deployment and gated on
/// `VIEW_INSTANCES`, because a usage graph is read directly by whoever asks,
/// with no other check upstream of it.
pub trait MetricsService: Send + Sync {
    fn record_bucket(
        &self,
        command: RecordMetricBucketCommand,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn usage_for_deployment(
        &self,
        identity: Identity,
        query: DeploymentUsageQuery,
    ) -> impl Future<Output = Result<MetricSeries, CoreError>> + Send;

    /// The active user count for one deployment, or `None` when it has not
    /// reported inside the counting window. See [`crate::metrics::active_users`]
    /// for the one definition this delegates to.
    fn active_users_for_deployment(
        &self,
        identity: Identity,
        query: ActiveUsersQuery,
    ) -> impl Future<Output = Result<Option<u64>, CoreError>> + Send;
}
