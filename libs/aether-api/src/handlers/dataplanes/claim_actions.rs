use aether_auth::Identity;
use aether_core::{
    action::{Action, commands::ClaimActionsCommand},
    dataplane::value_objects::DataPlaneId,
    deployments::DeploymentId,
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/dataplanes/{dataplane_id}/actions:claim")]
pub struct ClaimActionRoute {
    pub dataplane_id: DataPlaneId,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ClaimActionsResponse {
    pub data: Vec<Action>,
}

/// `deployment_ids` is the caller's shard, already resolved to the concrete
/// deployments it owns: one call claims every one of them, rather than one
/// call per deployment.
#[derive(Deserialize, ToSchema)]
pub struct ClaimActionsRequest {
    pub deployment_ids: Vec<DeploymentId>,
    pub max: usize,
    pub lease_seconds: u64,
}

#[utoipa::path(
    post,
    path = "/{dataplane_id}/actions:claim",
    summary = "claim actions",
    tag = "dataplanes",
    request_body = ClaimActionsRequest,
    description = "Claim actions for the given deployments on the dataplane, in one call.",
    params(ClaimActionRoute),
    responses(
        (status = 200, description = "Claimed actions", body = ClaimActionsResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 400, description = "Invalid dataplane id", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn claim_actions_handler(
    ClaimActionRoute { dataplane_id }: ClaimActionRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<ClaimActionsRequest>,
) -> Result<Response<ClaimActionsResponse>, ApiError> {
    let actions = state
        .service
        .claim_actions(
            identity,
            ClaimActionsCommand {
                dataplane_id,
                deployment_ids: request.deployment_ids,
                max: request.max,
                lease_seconds: request.lease_seconds as i64,
            },
        )
        .await?;

    Ok(Response::OK(ClaimActionsResponse { data: actions }))
}
