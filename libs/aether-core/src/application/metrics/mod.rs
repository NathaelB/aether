use aether_auth::Identity;
use aether_domain::{
    CoreError,
    metrics::{
        MetricSeries,
        commands::{ActiveUsersQuery, DeploymentUsageQuery, RecordMetricBucketCommand},
        ports::MetricsService,
        service::MetricsServiceImpl,
    },
};
use aether_macros::transactional;

use crate::{AetherService, infrastructure::role::permissions_in};

impl MetricsService for AetherService {
    #[transactional(metrics)]
    async fn record_bucket(&self, command: RecordMetricBucketCommand) -> Result<(), CoreError> {
        MetricsServiceImpl::new(metrics_repository, permissions_in(&tx))
            .record_bucket(command)
            .await
    }

    #[transactional(metrics)]
    async fn usage_for_deployment(
        &self,
        identity: Identity,
        query: DeploymentUsageQuery,
    ) -> Result<MetricSeries, CoreError> {
        MetricsServiceImpl::new(metrics_repository, permissions_in(&tx))
            .usage_for_deployment(identity, query)
            .await
    }

    #[transactional(metrics)]
    async fn active_users_for_deployment(
        &self,
        identity: Identity,
        query: ActiveUsersQuery,
    ) -> Result<Option<u64>, CoreError> {
        MetricsServiceImpl::new(metrics_repository, permissions_in(&tx))
            .active_users_for_deployment(identity, query)
            .await
    }
}
