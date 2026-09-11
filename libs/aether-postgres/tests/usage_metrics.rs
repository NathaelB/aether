use aether_domain::{
    CoreError,
    deployments::DeploymentId,
    metrics::{
        MetricBucket, MetricKind, MetricPoint, TimeRange,
        ports::{MetricsReadRepository, MetricsWriteRepository},
    },
    organisation::OrganisationId,
};
use aether_persistence::with_tx;
use aether_postgres::metrics::PostgresMetricsRepository;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

mod support;
use support::pool;

/// Each test owns an organisation, a data plane and a deployment nobody else
/// writes, so the suite stays parallel against a shared database.
struct Fixture {
    tag: String,
    organisation_id: OrganisationId,
    deployment_id: DeploymentId,
    dataplane_id: Uuid,
    user_id: Uuid,
}

impl Fixture {
    fn new(label: &str) -> Self {
        Self {
            tag: format!("usage-metrics-{label}-{}", Uuid::new_v4()),
            organisation_id: OrganisationId(Uuid::new_v4()),
            deployment_id: DeploymentId(Uuid::new_v4()),
            dataplane_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
        }
    }

    fn point(&self, metric: MetricKind, at: DateTime<Utc>, value: u64) -> MetricPoint {
        MetricPoint {
            deployment_id: self.deployment_id,
            metric,
            bucket: MetricBucket::containing(at),
            value,
        }
    }
}

fn map_err(e: sqlx::Error) -> CoreError {
    CoreError::DatabaseError {
        message: e.to_string(),
    }
}

fn base() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-06-01T12:00:00Z")
        .expect("a valid instant")
        .with_timezone(&Utc)
}

async fn clean(pool: &PgPool, fixture: &Fixture) {
    for statement in [
        "DELETE FROM deployments WHERE dataplane_id IN (SELECT id FROM data_planes WHERE region = $1)",
        "DELETE FROM data_planes WHERE region = $1",
        "DELETE FROM organisations WHERE name = $1",
        "DELETE FROM users WHERE name = $1",
    ] {
        sqlx::query(statement)
            .bind(&fixture.tag)
            .execute(pool)
            .await
            .expect("cleanup");
    }
}

/// Seeds a user, an organisation, a shared data plane and a deployment.
/// Deleting the deployment on cleanup cascades to whatever usage_metrics rows
/// the test wrote, since `usage_metrics.deployment_id` is `ON DELETE CASCADE`.
async fn seed(pool: &PgPool, fixture: &Fixture) {
    sqlx::query("INSERT INTO users (id, email, name, sub) VALUES ($1, $2, $3, $4)")
        .bind(fixture.user_id)
        .bind(format!("{}@usage-metrics.test", fixture.user_id))
        .bind(&fixture.tag)
        .bind(fixture.user_id.to_string())
        .execute(pool)
        .await
        .expect("user seeded");

    sqlx::query(
        "INSERT INTO organisations \
         (id, name, slug, owner_id, status, plan, max_instances, max_users, \
          max_storage_gb, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, 'active', 'free', 5, 5, 5, now(), now())",
    )
    .bind(fixture.organisation_id.0)
    .bind(&fixture.tag)
    .bind(fixture.organisation_id.0.to_string())
    .bind(fixture.user_id)
    .execute(pool)
    .await
    .expect("organisation seeded");

    sqlx::query(
        "INSERT INTO data_planes \
         (id, mode, region, status, capacity_cpu_millis, capacity_memory_mib, \
          capacity_storage_gib, created_at, updated_at) \
         VALUES ($1, 'shared', $2, 'active', 8000, 16384, 200, now(), now())",
    )
    .bind(fixture.dataplane_id)
    .bind(&fixture.tag)
    .execute(pool)
    .await
    .expect("data plane seeded");

    sqlx::query(
        "INSERT INTO deployments \
         (id, organisation_id, dataplane_id, name, kind, status, namespace, version, \
          cpu_millis, memory_mib, storage_gib, created_by, created_at, updated_at) \
         VALUES ($1, $2, $3, 'auth', 'ferriskey', 'successful', $4, '26.0.0', \
                 500, 1024, 1, $5, now(), now())",
    )
    .bind(fixture.deployment_id.0)
    .bind(fixture.organisation_id.0)
    .bind(fixture.dataplane_id)
    .bind(&fixture.tag)
    .bind(fixture.user_id)
    .execute(pool)
    .await
    .expect("deployment seeded");
}

fn wide_range() -> TimeRange {
    TimeRange::new(base() - Duration::hours(1), base() + Duration::hours(1)).expect("a sane range")
}

#[tokio::test]
async fn a_bucket_survives_a_round_trip() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new("round-trip");
    clean(&pool, &fixture).await;
    seed(&pool, &fixture).await;

    let point = fixture.point(MetricKind::Requests, base(), 42);

    let result = with_tx(&pool, map_err, async |tx| {
        let metrics = PostgresMetricsRepository::new(&tx);
        metrics.record_bucket(point.clone()).await?;
        metrics
            .series_for_deployment(
                fixture.organisation_id,
                fixture.deployment_id,
                MetricKind::Requests,
                wide_range(),
            )
            .await
    })
    .await;

    let series = result.expect("committed");
    clean(&pool, &fixture).await;

    assert_eq!(series.points.len(), 1);
    assert_eq!(series.points[0].bucket, point.bucket);
    assert_eq!(series.points[0].value, 42);
}

/// The acceptance criterion #121 exists for: Herald replays its report
/// window after a restart, and the retried report may be the more complete
/// figure for a bucket only half observed before the crash. A second write
/// must overwrite, not add.
#[tokio::test]
async fn a_bucket_reported_twice_is_not_counted_twice() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new("replay");
    clean(&pool, &fixture).await;
    seed(&pool, &fixture).await;

    let result = with_tx(&pool, map_err, async |tx| {
        let metrics = PostgresMetricsRepository::new(&tx);
        metrics
            .record_bucket(fixture.point(MetricKind::Logins, base(), 5))
            .await?;
        metrics
            .record_bucket(fixture.point(MetricKind::Logins, base(), 9))
            .await?;
        metrics
            .series_for_deployment(
                fixture.organisation_id,
                fixture.deployment_id,
                MetricKind::Logins,
                wide_range(),
            )
            .await
    })
    .await;

    let series = result.expect("committed");
    clean(&pool, &fixture).await;

    assert_eq!(
        series.points.len(),
        1,
        "a replay must not open a second row for the same bucket"
    );
    assert_eq!(
        series.points[0].value, 9,
        "the retried report wins over the interrupted one, and neither is summed"
    );
}

/// The acceptance criterion the read side exists for: a minute nothing
/// arrived for must be missing from the series, not present with a value of
/// zero the repository made up to fill the gap.
#[tokio::test]
async fn an_unreported_minute_is_absent_rather_than_a_reported_zero() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new("gap");
    clean(&pool, &fixture).await;
    seed(&pool, &fixture).await;

    let result = with_tx(&pool, map_err, async |tx| {
        let metrics = PostgresMetricsRepository::new(&tx);
        metrics
            .record_bucket(fixture.point(MetricKind::TokenEvents, base(), 3))
            .await?;
        // The minute at `base() + 1 minute` is deliberately never reported.
        metrics
            .record_bucket(fixture.point(MetricKind::TokenEvents, base() + Duration::minutes(2), 7))
            .await?;
        metrics
            .series_for_deployment(
                fixture.organisation_id,
                fixture.deployment_id,
                MetricKind::TokenEvents,
                wide_range(),
            )
            .await
    })
    .await;

    let series = result.expect("committed");
    clean(&pool, &fixture).await;

    assert_eq!(
        series.points.len(),
        2,
        "the skipped minute must not appear as a third, invented point"
    );
    let missing = MetricBucket::containing(base() + Duration::minutes(1));
    assert!(
        !series.points.iter().any(|point| point.bucket == missing),
        "the unreported minute must not be present at all, not even as zero"
    );
}

#[tokio::test]
async fn the_series_comes_back_oldest_first() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new("ordering");
    clean(&pool, &fixture).await;
    seed(&pool, &fixture).await;

    let result = with_tx(&pool, map_err, async |tx| {
        let metrics = PostgresMetricsRepository::new(&tx);
        for minutes in [3_i64, 1, 2] {
            metrics
                .record_bucket(fixture.point(
                    MetricKind::Requests,
                    base() + Duration::minutes(minutes),
                    minutes as u64,
                ))
                .await?;
        }
        metrics
            .series_for_deployment(
                fixture.organisation_id,
                fixture.deployment_id,
                MetricKind::Requests,
                wide_range(),
            )
            .await
    })
    .await;

    let series = result.expect("committed");
    clean(&pool, &fixture).await;

    assert_eq!(
        series
            .points
            .iter()
            .map(|point| point.value)
            .collect::<Vec<_>>(),
        vec![1, 2, 3],
        "oldest bucket first, regardless of write order"
    );
}

#[tokio::test]
async fn a_series_for_one_metric_does_not_include_another_metrics_buckets() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new("metric-isolation");
    clean(&pool, &fixture).await;
    seed(&pool, &fixture).await;

    let result = with_tx(&pool, map_err, async |tx| {
        let metrics = PostgresMetricsRepository::new(&tx);
        metrics
            .record_bucket(fixture.point(MetricKind::Requests, base(), 100))
            .await?;
        metrics
            .record_bucket(fixture.point(MetricKind::Logins, base(), 4))
            .await?;
        metrics
            .series_for_deployment(
                fixture.organisation_id,
                fixture.deployment_id,
                MetricKind::Logins,
                wide_range(),
            )
            .await
    })
    .await;

    let series = result.expect("committed");
    clean(&pool, &fixture).await;

    assert_eq!(series.points.len(), 1);
    assert_eq!(series.points[0].value, 4);
}

/// The acceptance criterion the org scoping exists for: a deployment read
/// through an organisation it does not belong to must come back with no
/// points, the same as a deployment with no data yet -- never with the other
/// organisation's numbers.
#[tokio::test]
async fn a_deployment_read_through_another_organisation_comes_back_with_no_points() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new("cross-tenant");
    clean(&pool, &fixture).await;
    seed(&pool, &fixture).await;
    let someone_elses_organisation = OrganisationId(Uuid::new_v4());

    let result = with_tx(&pool, map_err, async |tx| {
        let metrics = PostgresMetricsRepository::new(&tx);
        metrics
            .record_bucket(fixture.point(MetricKind::ActiveUsers, base(), 12))
            .await?;
        metrics
            .series_for_deployment(
                someone_elses_organisation,
                fixture.deployment_id,
                MetricKind::ActiveUsers,
                wide_range(),
            )
            .await
    })
    .await;

    let series = result.expect("committed");
    clean(&pool, &fixture).await;

    assert!(
        series.points.is_empty(),
        "a deployment must never be readable through an organisation it does not belong to"
    );
}
