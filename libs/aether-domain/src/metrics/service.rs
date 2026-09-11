use aether_auth::Identity;
use aether_permission::Permissions;

use crate::{
    CoreError,
    metrics::{
        MetricKind, MetricPoint, MetricSeries, TimeRange, active_users,
        commands::{ActiveUsersQuery, DeploymentUsageQuery, RecordMetricBucketCommand},
        ports::{MetricsReadRepository, MetricsService, MetricsWriteRepository},
    },
    organisation::OrganisationId,
    role::ports::PermissionProvider,
};

pub struct MetricsServiceImpl<R, P>
where
    R: MetricsWriteRepository + MetricsReadRepository,
    P: PermissionProvider,
{
    metrics_repository: R,
    permission_provider: P,
}

impl<R, P> MetricsServiceImpl<R, P>
where
    R: MetricsWriteRepository + MetricsReadRepository,
    P: PermissionProvider,
{
    pub fn new(metrics_repository: R, permission_provider: P) -> Self {
        Self {
            metrics_repository,
            permission_provider,
        }
    }

    async fn require_view_instances(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> Result<(), CoreError> {
        let permissions = self
            .permission_provider
            .permissions_for_organisation(identity, organisation_id)
            .await?;

        if !permissions.can(Permissions::VIEW_INSTANCES) {
            return Err(CoreError::PermissionDenied {
                reason: "viewing usage metrics requires VIEW_INSTANCES".to_string(),
            });
        }

        Ok(())
    }
}

impl<R, P> MetricsService for MetricsServiceImpl<R, P>
where
    R: MetricsWriteRepository + MetricsReadRepository,
    P: PermissionProvider,
{
    async fn record_bucket(&self, command: RecordMetricBucketCommand) -> Result<(), CoreError> {
        let point = MetricPoint {
            deployment_id: command.deployment_id,
            metric: command.metric,
            bucket: command.bucket,
            value: command.value,
        };

        self.metrics_repository.record_bucket(point).await
    }

    async fn usage_for_deployment(
        &self,
        identity: Identity,
        query: DeploymentUsageQuery,
    ) -> Result<MetricSeries, CoreError> {
        self.require_view_instances(identity, query.organisation_id)
            .await?;

        let range = query.range()?;
        self.metrics_repository
            .series_for_deployment(
                query.organisation_id,
                query.deployment_id,
                query.metric,
                range,
            )
            .await
    }

    async fn active_users_for_deployment(
        &self,
        identity: Identity,
        query: ActiveUsersQuery,
    ) -> Result<Option<u64>, CoreError> {
        self.require_view_instances(identity, query.organisation_id)
            .await?;

        let range = TimeRange::new(query.at - query.window, query.at)?;
        let series = self
            .metrics_repository
            .series_for_deployment(
                query.organisation_id,
                query.deployment_id,
                MetricKind::ActiveUsers,
                range,
            )
            .await?;

        Ok(active_users(&series.points, query.at, query.window))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deployments::DeploymentId;
    use crate::metrics::ports::{MockMetricsReadRepository, MockMetricsWriteRepository};
    use crate::metrics::{MetricBucket, ReportedBucket};
    use crate::role::ports::MockPermissionProvider;
    use aether_auth::User;
    use chrono::{DateTime, Duration, Utc};
    use uuid::Uuid;

    /// `MetricsServiceImpl` needs one type that is both a write and a read
    /// repository, matching the concrete Postgres adapter. Mockall generates
    /// a separate mock struct per trait, so this delegates to one of each.
    #[derive(Default)]
    struct TestRepository {
        write: MockMetricsWriteRepository,
        read: MockMetricsReadRepository,
    }

    impl MetricsWriteRepository for TestRepository {
        async fn record_bucket(&self, point: MetricPoint) -> Result<(), CoreError> {
            self.write.record_bucket(point).await
        }
    }

    impl MetricsReadRepository for TestRepository {
        async fn series_for_deployment(
            &self,
            organisation_id: OrganisationId,
            deployment_id: DeploymentId,
            metric: MetricKind,
            range: TimeRange,
        ) -> Result<MetricSeries, CoreError> {
            self.read
                .series_for_deployment(organisation_id, deployment_id, metric, range)
                .await
        }
    }

    fn identity() -> Identity {
        Identity::User(User {
            id: "user-1".to_string(),
            username: "user".to_string(),
            email: None,
            name: None,
            roles: vec![],
        })
    }

    fn allow(permissions: Permissions) -> MockPermissionProvider {
        let mut provider = MockPermissionProvider::new();
        provider
            .expect_permissions_for_organisation()
            .returning(move |_, _| {
                let permissions = permissions;
                Box::pin(async move { Ok(permissions) })
            });
        provider
    }

    fn at() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .expect("a valid instant")
            .with_timezone(&Utc)
    }

    #[tokio::test]
    async fn recording_a_bucket_reaches_the_repository_without_a_permission_check() {
        let mut write = MockMetricsWriteRepository::new();
        write
            .expect_record_bucket()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));

        let repository = TestRepository {
            write,
            read: MockMetricsReadRepository::new(),
        };
        let service = MetricsServiceImpl::new(repository, allow(Permissions::empty()));

        let command = RecordMetricBucketCommand::new(
            DeploymentId(Uuid::new_v4()),
            MetricKind::Requests,
            at(),
            42,
        );

        assert!(service.record_bucket(command).await.is_ok());
    }

    #[tokio::test]
    async fn reading_usage_without_view_instances_is_refused() {
        let repository = TestRepository::default();
        let service = MetricsServiceImpl::new(repository, allow(Permissions::empty()));

        let query = DeploymentUsageQuery::new(
            OrganisationId(Uuid::new_v4()),
            DeploymentId(Uuid::new_v4()),
            MetricKind::Requests,
            at() - Duration::hours(1),
            at(),
        );

        let result = service.usage_for_deployment(identity(), query).await;
        assert!(matches!(result, Err(CoreError::PermissionDenied { .. })));
    }

    /// A read mock that hands back an empty series for whatever deployment
    /// and metric it is asked for, so a test can focus on the permission
    /// check rather than on the data.
    fn read_repository_returning_an_empty_series() -> MockMetricsReadRepository {
        let mut read = MockMetricsReadRepository::new();
        read.expect_series_for_deployment().times(1).returning(
            |_organisation_id, deployment_id, metric, _range| {
                Box::pin(async move {
                    Ok(MetricSeries {
                        deployment_id,
                        metric,
                        points: vec![],
                    })
                })
            },
        );
        read
    }

    #[tokio::test]
    async fn reading_usage_with_view_instances_reaches_the_repository() {
        let repository = TestRepository {
            write: MockMetricsWriteRepository::new(),
            read: read_repository_returning_an_empty_series(),
        };
        let service = MetricsServiceImpl::new(repository, allow(Permissions::VIEW_INSTANCES));

        let query = DeploymentUsageQuery::new(
            OrganisationId(Uuid::new_v4()),
            DeploymentId(Uuid::new_v4()),
            MetricKind::Requests,
            at() - Duration::hours(1),
            at(),
        );

        let result = service.usage_for_deployment(identity(), query).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn an_administrator_may_read_usage_without_the_named_permission() {
        let repository = TestRepository {
            write: MockMetricsWriteRepository::new(),
            read: read_repository_returning_an_empty_series(),
        };
        let service = MetricsServiceImpl::new(repository, allow(Permissions::ADMINISTRATOR));

        let query = DeploymentUsageQuery::new(
            OrganisationId(Uuid::new_v4()),
            DeploymentId(Uuid::new_v4()),
            MetricKind::Requests,
            at() - Duration::hours(1),
            at(),
        );

        let result = service.usage_for_deployment(identity(), query).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn active_users_without_view_instances_is_refused() {
        let repository = TestRepository::default();
        let service = MetricsServiceImpl::new(repository, allow(Permissions::empty()));

        let query = ActiveUsersQuery::new(
            OrganisationId(Uuid::new_v4()),
            DeploymentId(Uuid::new_v4()),
            at(),
            Duration::minutes(30),
        );

        let result = service.active_users_for_deployment(identity(), query).await;
        assert!(matches!(result, Err(CoreError::PermissionDenied { .. })));
    }

    /// This is the service-level guarantee #123 is really about: whatever the
    /// repository hands back, the count that reaches a caller is the one
    /// [`crate::metrics::active_users`] produces, not a sum computed here.
    #[tokio::test]
    async fn active_users_for_a_deployment_delegates_to_the_one_definition() {
        let deployment_id = DeploymentId(Uuid::new_v4());
        let mut read = MockMetricsReadRepository::new();
        read.expect_series_for_deployment().times(1).returning(
            move |organisation_id, deployment_id, metric, _range| {
                let _ = organisation_id;
                Box::pin(async move {
                    Ok(MetricSeries {
                        deployment_id,
                        metric,
                        points: vec![
                            ReportedBucket {
                                bucket: MetricBucket::containing(at() - Duration::minutes(20)),
                                value: 3,
                            },
                            ReportedBucket {
                                bucket: MetricBucket::containing(at() - Duration::minutes(5)),
                                value: 8,
                            },
                        ],
                    })
                })
            },
        );

        let repository = TestRepository {
            write: MockMetricsWriteRepository::new(),
            read,
        };
        let service = MetricsServiceImpl::new(repository, allow(Permissions::VIEW_INSTANCES));

        let query = ActiveUsersQuery::new(
            OrganisationId(Uuid::new_v4()),
            deployment_id,
            at(),
            Duration::minutes(30),
        );

        let result = service
            .active_users_for_deployment(identity(), query)
            .await
            .expect("permitted");

        assert_eq!(
            result,
            Some(8),
            "the most recent bucket, not the sum of both"
        );
    }
}
