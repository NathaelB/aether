use aether_core::{
    action::{Action, ActionId, ports::ActionService},
    deployments::ports::DeploymentService,
    organisation::OrganisationId,
};
use axum::extract::State;
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Serialize, ToSchema, PartialEq)]
pub struct GetActionResponse {
    data: Action,
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments/{deployment_id}/actions/{action_id}")]
pub struct GetActionRoute {
    pub organisation_id: Uuid,
    pub deployment_id: Uuid,
    pub action_id: Uuid,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/deployments/{deployment_id}/actions/{action_id}",
    summary = "get action",
    tag = "actions",
    description = "Retrieve a single action for a deployment within the specified organisation.",
    params(GetActionRoute),
    responses(
        (status = 200, description = "Action details", body = GetActionResponse),
        (status = 400, description = "Action not found", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    )
)]
pub async fn get_action_handler(
    GetActionRoute {
        organisation_id,
        deployment_id,
        action_id,
    }: GetActionRoute,
    State(state): State<AppState>,
) -> Result<Response<GetActionResponse>, ApiError> {
    let organisation_id = OrganisationId(organisation_id);
    let deployment_id = deployment_id.into();

    state
        .service
        .get_deployment_for_organisation(organisation_id, deployment_id)
        .await?;

    let action = state
        .service
        .get_action(deployment_id, ActionId(action_id))
        .await?
        .ok_or(ApiError::BadRequest {
            reason: "action not found".to_string(),
        })?;

    Ok(Response::OK(GetActionResponse { data: action }))
}
