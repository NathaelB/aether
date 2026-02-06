use aether_auth::Identity;
use aether_core::{
    action::{Action, commands::ClaimActionsCommand, ports::ActionService},
    dataplane::value_objects::DataPlaneId,
    deployments::DeploymentId,
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, Deserialize)]
#[typed_path("/dataplanes/{dataplane_id}/deployments/{deployment_id}/actions:claim")]
pub struct ClaimActionRoute {
    pub dataplane_id: DataPlaneId,
    pub deployment_id: DeploymentId,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ClaimActionsResponse {
    pub data: Vec<Action>,
}

#[derive(Deserialize, ToSchema)]
pub struct ClaimActionsRequest {
    pub max: usize,
    pub lease_seconds: u64,
}

#[utoipa::path(
    post,
    path = "/{dataplane_id}/deployments/{deployment_id}/actions:claim",
    summary = "claim actions",
    tag = "dataplanes",
    request_body = ClaimActionsRequest,
    description = "Claim actions for the specified deployment on the dataplane.",
    responses(
        (status = 200, description = "Claimed actions", body = ClaimActionsResponse),
        (status = 400, description = "Invalid dataplane or deployment id", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    )
)]
pub async fn claim_actions_handler(
    ClaimActionRoute {
        dataplane_id,
        deployment_id,
    }: ClaimActionRoute,
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
                deployment_id,
                max: request.max,
                lease_seconds: request.lease_seconds as i64,
            },
        )
        .await?;

    Ok(Response::OK(ClaimActionsResponse { data: actions }))
}
