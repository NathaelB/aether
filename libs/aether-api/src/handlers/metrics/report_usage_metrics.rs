use aether_core::{
    deployments::DeploymentId,
    metrics::{MetricKind, commands::RecordMetricBucketCommand, ports::MetricsService},
};
use axum::{Json, extract::State};
use axum_extra::routing::TypedPath;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/deployments/{deployment_id}/usage-metrics")]
pub struct ReportUsageMetricsRoute {
    pub deployment_id: DeploymentId,
}

#[derive(Deserialize, ToSchema)]
pub struct ReportedMetricPoint {
    /// One of `requests`, `token_events`, `logins`, `active_users`.
    pub metric: String,
    /// The minute this point covers. Floored to the minute on arrival, so a
    /// stray sub-minute timestamp does not open a second row for what is
    /// already the same bucket.
    pub bucket: DateTime<Utc>,
    pub value: u64,
}

#[derive(Deserialize, ToSchema)]
pub struct ReportUsageMetricsRequest {
    pub points: Vec<ReportedMetricPoint>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ReportUsageMetricsResponseData {
    pub recorded: usize,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ReportUsageMetricsResponse {
    data: ReportUsageMetricsResponseData,
}

#[utoipa::path(
    post,
    path = "/{deployment_id}/usage-metrics",
    summary = "report a batch of usage metric buckets for a deployment",
    tag = "metrics",
    description = "Records the buckets a data plane observed for its own deployment. \
                   Idempotent on (deployment, metric, bucket): a bucket reported again \
                   overwrites the value rather than adding to it, since Herald replays its \
                   window after a restart. The whole batch is validated before anything is \
                   written, so one unknown metric name in a batch cannot leave the rest \
                   half recorded.",
    params(ReportUsageMetricsRoute),
    request_body = ReportUsageMetricsRequest,
    responses(
        (status = 200, description = "Buckets recorded", body = ReportUsageMetricsResponse),
        (status = 400, description = "An unknown metric name", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn report_usage_metrics_handler(
    ReportUsageMetricsRoute { deployment_id }: ReportUsageMetricsRoute,
    State(state): State<AppState>,
    Json(request): Json<ReportUsageMetricsRequest>,
) -> Result<Response<ReportUsageMetricsResponse>, ApiError> {
    let commands = request
        .points
        .iter()
        .map(|point| {
            MetricKind::try_from(point.metric.as_str())
                .map(|metric| {
                    RecordMetricBucketCommand::new(deployment_id, metric, point.bucket, point.value)
                })
                .map_err(|_| ApiError::BadRequest {
                    reason: format!("unknown metric '{}'", point.metric),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut recorded = 0usize;
    for command in commands {
        state.service.record_bucket(command).await?;
        recorded += 1;
    }

    Ok(Response::OK(ReportUsageMetricsResponse {
        data: ReportUsageMetricsResponseData { recorded },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::app_state;
    use uuid::Uuid;

    /// A batch is checked in full before anything is written: this must fail
    /// without ever reaching the service for a valid point placed after the
    /// bad one.
    #[tokio::test]
    async fn an_unknown_metric_name_is_rejected_before_anything_is_recorded() {
        let state = app_state();

        let result = report_usage_metrics_handler(
            ReportUsageMetricsRoute {
                deployment_id: DeploymentId(Uuid::new_v4()),
            },
            State(state),
            Json(ReportUsageMetricsRequest {
                points: vec![
                    ReportedMetricPoint {
                        metric: "not_a_real_metric".to_string(),
                        bucket: Utc::now(),
                        value: 1,
                    },
                    ReportedMetricPoint {
                        metric: "requests".to_string(),
                        bucket: Utc::now(),
                        value: 1,
                    },
                ],
            }),
        )
        .await;

        assert!(matches!(result, Err(ApiError::BadRequest { .. })));
    }
}
