use aether_auth::Identity;
use aether_core::deployments::DeploymentId;
use aether_core::deployments::reachability_history::DeploymentDowntime;
use axum::Extension;
use axum::extract::{Query, State};
use axum_extra::routing::TypedPath;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{errors::ApiError, response::Response, state::AppState};

const DEFAULT_WINDOW_DAYS: u32 = 30;
const MAX_WINDOW_DAYS: u32 = 90;

fn default_days() -> u32 {
    DEFAULT_WINDOW_DAYS
}

#[derive(Serialize, Deserialize, ToSchema, PartialEq, Debug, Clone)]
pub struct DeploymentDowntimeResponse {
    data: DeploymentDowntime,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DeploymentDowntimeQuery {
    /// Window length in days, 30 by default and capped at 90.
    #[serde(default = "default_days")]
    days: u32,
}

#[derive(TypedPath, Deserialize, IntoParams, Clone)]
#[typed_path("/platform/deployments/{deployment_id}/downtime")]
pub struct DeploymentDowntimeRoute {
    deployment_id: DeploymentId,
}

#[utoipa::path(
    get,
    path = "/deployments/{deployment_id}/downtime",
    summary = "get deployment downtime intervals",
    tag = "platform",
    description = "Periods during which a deployment did not answer, derived from recorded \
                   reachability checks. An interval runs from the first failed check to the \
                   first successful one after it, and is open while the deployment is still \
                   failing. Requires view_estate.",
    params(DeploymentDowntimeRoute, DeploymentDowntimeQuery),
    responses(
        (status = 200, description = "Downtime intervals", body = DeploymentDowntimeResponse),
        (status = 400, description = "Invalid deployment ID", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller does not hold view_estate", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_deployment_downtime_handler(
    DeploymentDowntimeRoute { deployment_id }: DeploymentDowntimeRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Query(query): Query<DeploymentDowntimeQuery>,
) -> Result<Response<DeploymentDowntimeResponse>, ApiError> {
    let since = Utc::now() - window(query.days);

    let downtime = state
        .service
        .get_deployment_downtime(identity, deployment_id, since)
        .await?;

    Ok(Response::OK(DeploymentDowntimeResponse { data: downtime }))
}

fn window(days: u32) -> Duration {
    Duration::days(i64::from(days.clamp(1, MAX_WINDOW_DAYS)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{app_state, user_identity};
    use aether_core::deployments::reachability_history::DowntimeInterval;

    #[tokio::test]
    async fn get_deployment_downtime_maps_service_error() {
        let result = get_deployment_downtime_handler(
            DeploymentDowntimeRoute {
                deployment_id: DeploymentId(uuid::Uuid::nil()),
            },
            State(app_state()),
            Extension(user_identity("operator-1")),
            Query(DeploymentDowntimeQuery { days: 30 }),
        )
        .await;

        assert!(matches!(result, Err(ApiError::Unknown { .. })));
    }

    #[test]
    fn window_is_capped_and_never_empty() {
        assert_eq!(window(30), Duration::days(30));
        assert_eq!(window(1000), Duration::days(90));
        assert_eq!(window(0), Duration::days(1));
    }

    #[test]
    fn response_serializes_open_and_closed_intervals() {
        let started_at = chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let response = DeploymentDowntimeResponse {
            data: DeploymentDowntime {
                deployment_id: DeploymentId(uuid::Uuid::nil()),
                intervals: vec![
                    DowntimeInterval {
                        started_at,
                        ended_at: Some(started_at + Duration::seconds(60)),
                        duration_seconds: Some(60),
                    },
                    DowntimeInterval {
                        started_at,
                        ended_at: None,
                        duration_seconds: None,
                    },
                ],
            },
        };

        let json = serde_json::to_value(&response).unwrap();
        let intervals = &json["data"]["intervals"];
        assert_eq!(intervals[0]["duration_seconds"], 60);
        assert!(intervals[1]["ended_at"].is_null());
        assert!(intervals[1]["duration_seconds"].is_null());
    }
}
