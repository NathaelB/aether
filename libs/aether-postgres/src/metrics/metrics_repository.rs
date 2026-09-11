use chrono::{DateTime, Utc};
use sqlx::FromRow;

use aether_domain::{
    CoreError,
    deployments::DeploymentId,
    metrics::{
        MetricBucket, MetricKind, MetricPoint, MetricSeries, ReportedBucket, TimeRange,
        ports::{MetricsReadRepository, MetricsWriteRepository},
    },
    organisation::OrganisationId,
};
use aether_macros::repository;
use aether_persistence::SharedTx;

/// Only the two columns that vary per row: `deployment_id` and `metric` are
/// already known to the caller (they chose what to ask for), so there is
/// nothing to reconstruct them from.
#[derive(FromRow)]
struct MetricRow {
    bucket_start: DateTime<Utc>,
    value: i64,
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[repository(domain = Metrics, backend = Postgres)]
pub struct PostgresMetricsRepository<'tx> {
    tx: SharedTx<'tx>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'tx> PostgresMetricsRepository<'tx> {
    pub fn new(tx: &SharedTx<'tx>) -> Self {
        Self { tx: tx.clone() }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl MetricsWriteRepository for PostgresMetricsRepository<'_> {
    async fn record_bucket(&self, point: MetricPoint) -> Result<(), CoreError> {
        let metric = point.metric.to_string();
        let bucket_start = point.bucket.start();
        let value = point.value as i64;

        {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            INSERT INTO usage_metrics (deployment_id, metric, bucket_start, value)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (deployment_id, metric, bucket_start)
            DO UPDATE SET value = EXCLUDED.value
            "#,
                point.deployment_id.0,
                metric,
                bucket_start,
                value,
            )
            .execute(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to record usage metric: {e}"),
        })?;

        Ok(())
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl MetricsReadRepository for PostgresMetricsRepository<'_> {
    async fn series_for_deployment(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
        metric: MetricKind,
        range: TimeRange,
    ) -> Result<MetricSeries, CoreError> {
        let metric_name = metric.to_string();

        let rows = {
            let mut tx = self.tx.lock().await;
            sqlx::query_as!(
                MetricRow,
                r#"
            SELECT um.bucket_start, um.value
            FROM usage_metrics um
            JOIN deployments d ON d.id = um.deployment_id
            WHERE um.deployment_id = $1
              AND d.organisation_id = $2
              AND um.metric = $3
              AND um.bucket_start >= $4
              AND um.bucket_start <= $5
            ORDER BY um.bucket_start ASC
            "#,
                deployment_id.0,
                organisation_id.0,
                metric_name,
                range.from(),
                range.until(),
            )
            .fetch_all(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to list usage metrics: {e}"),
        })?;

        let points = rows
            .into_iter()
            .map(|row| ReportedBucket {
                bucket: MetricBucket::containing(row.bucket_start),
                value: row.value as u64,
            })
            .collect();

        Ok(MetricSeries {
            deployment_id,
            metric,
            points,
        })
    }
}
