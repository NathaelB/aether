use aether_auth::Identity;
use aether_core::{
    deployments::DeploymentId,
    metrics::{MetricKind, commands::DeploymentUsageQuery, ports::MetricsService},
    organisation::OrganisationId,
};
use axum::{
    Extension,
    extract::{Query, State},
};
use axum_extra::routing::TypedPath;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments/{deployment_id}/usage-metrics/{metric}")]
pub struct GetDeploymentUsageRoute {
    pub organisation_id: Uuid,
    pub deployment_id: Uuid,
    pub metric: String,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct GetDeploymentUsageQuery {
    pub from: DateTime<Utc>,
    pub until: DateTime<Utc>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct UsageBucketResponse {
    pub bucket: DateTime<Utc>,
    pub value: u64,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct GetDeploymentUsageResponse {
    /// Only the buckets the data plane actually reported. A minute missing
    /// from this list is not a zero: it means nothing arrived for it, and the
    /// console must show that differently from "reported, and it was zero".
    data: Vec<UsageBucketResponse>,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/deployments/{deployment_id}/usage-metrics/{metric}",
    summary = "read a deployment's usage series for one metric",
    tag = "metrics",
    description = "Every bucket a deployment's data plane reported for one metric within a \
                   range, oldest first. A minute with no row is absent from the response \
                   rather than reported as zero. Requires VIEW_INSTANCES.",
    params(GetDeploymentUsageRoute, GetDeploymentUsageQuery),
    responses(
        (status = 200, description = "The deployment's usage series", body = GetDeploymentUsageResponse),
        (status = 400, description = "An unknown metric name or an invalid range", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "Forbidden", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_deployment_usage_handler(
    GetDeploymentUsageRoute {
        organisation_id,
        deployment_id,
        metric,
    }: GetDeploymentUsageRoute,
    Query(query): Query<GetDeploymentUsageQuery>,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<GetDeploymentUsageResponse>, ApiError> {
    let metric = MetricKind::try_from(metric.as_str()).map_err(|_| ApiError::BadRequest {
        reason: format!("unknown metric '{metric}'"),
    })?;

    let usage_query = DeploymentUsageQuery::new(
        OrganisationId(organisation_id),
        DeploymentId(deployment_id),
        metric,
        query.from,
        query.until,
    );

    let series = state
        .service
        .usage_for_deployment(identity, usage_query)
        .await?;

    Ok(Response::OK(GetDeploymentUsageResponse {
        data: series
            .points
            .into_iter()
            .map(|point| UsageBucketResponse {
                bucket: point.bucket.start(),
                value: point.value,
            })
            .collect(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{app_state, user_identity};

    #[tokio::test]
    async fn an_unknown_metric_name_is_rejected() {
        let state = app_state();

        let result = get_deployment_usage_handler(
            GetDeploymentUsageRoute {
                organisation_id: Uuid::new_v4(),
                deployment_id: Uuid::new_v4(),
                metric: "not_a_real_metric".to_string(),
            },
            Query(GetDeploymentUsageQuery {
                from: Utc::now(),
                until: Utc::now(),
            }),
            State(state),
            Extension(user_identity("user-1")),
        )
        .await;

        assert!(matches!(result, Err(ApiError::BadRequest { .. })));
    }
}
