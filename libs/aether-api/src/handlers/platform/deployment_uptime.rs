use aether_auth::Identity;
use aether_core::deployments::DeploymentId;
use aether_core::deployments::reachability_history::DeploymentUptime;
use axum::Extension;
use axum::extract::State;
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Serialize, Deserialize, ToSchema, PartialEq, Debug, Clone)]
pub struct DeploymentUptimeResponse {
    data: DeploymentUptime,
}

#[derive(TypedPath, Deserialize, IntoParams, Clone)]
#[typed_path("/platform/deployments/{deployment_id}/uptime")]
pub struct DeploymentUptimeRoute {
    deployment_id: DeploymentId,
}

#[utoipa::path(
    get,
    path = "/{deployment_id}/uptime",
    summary = "get deployment uptime metrics",
    tag = "platform",
    description = "Uptime percentages for a deployment over 24h, 7d, and 30d windows. \
                   Computed from recorded health checks. Requires view_estate.",
    params(DeploymentUptimeRoute),
    responses(
        (status = 200, description = "Uptime metrics", body = DeploymentUptimeResponse),
        (status = 400, description = "Invalid deployment ID", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller does not hold view_estate", body = ApiError),
        (status = 404, description = "Deployment not found or no checks recorded", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_deployment_uptime_handler(
    DeploymentUptimeRoute { deployment_id }: DeploymentUptimeRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<DeploymentUptimeResponse>, ApiError> {
    // Get the uptime metrics
    let uptime = state
        .service
        .get_deployment_uptime(identity, deployment_id)
        .await?;

    Ok(Response::OK(DeploymentUptimeResponse { data: uptime }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{app_state, user_identity};

    #[tokio::test]
    async fn get_deployment_uptime_maps_service_error() {
        let result = get_deployment_uptime_handler(
            DeploymentUptimeRoute {
                deployment_id: DeploymentId(uuid::Uuid::nil()),
            },
            State(app_state()),
            Extension(user_identity("operator-1")),
        )
        .await;

        assert!(matches!(result, Err(ApiError::Unknown { .. })));
    }

    #[test]
    fn deployment_uptime_response_serializes() {
        use aether_core::deployments::reachability_history::UptimeWindow;

        let response = DeploymentUptimeResponse {
            data: DeploymentUptime {
                deployment_id: DeploymentId(uuid::Uuid::nil()),
                uptime_24h: UptimeWindow {
                    uptime_percent: 100.0,
                    covers_full_window: true,
                },
                uptime_7d: UptimeWindow {
                    uptime_percent: 99.9,
                    covers_full_window: true,
                },
                uptime_30d: UptimeWindow {
                    uptime_percent: 99.5,
                    covers_full_window: true,
                },
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("100"));
    }
}
