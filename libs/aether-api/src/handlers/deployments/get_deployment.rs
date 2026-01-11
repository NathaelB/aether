use aether_core::deployments::{Deployment, ports::DeploymentService};
use axum::extract::State;
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Serialize, ToSchema, PartialEq)]
pub struct GetDeploymentResponse {
    data: Deployment,
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments/{deployment_id}")]
pub struct GetDeploymentRoute {
    pub organisation_id: Uuid,
    pub deployment_id: Uuid,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/deployments/{deployment_id}",
    summary = "get deployment",
    tag = "deployments",
    description = "Retrieve a deployment within the specified organisation.",
    params(GetDeploymentRoute),
    responses(
        (status = 200, description = "Deployment details", body = GetDeploymentResponse),
        (status = 400, description = "Deployment not found", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_deployment_handler(
    GetDeploymentRoute {
        organisation_id,
        deployment_id,
    }: GetDeploymentRoute,
    State(state): State<AppState>,
) -> Result<Response<GetDeploymentResponse>, ApiError> {
    let organisation_id = organisation_id.into();
    let deployment_id = deployment_id.into();

    let deployment = state
        .service
        .get_deployment_for_organisation(organisation_id, deployment_id)
        .await
        .map_err(|_| ApiError::BadRequest {
            reason: "deployment not found".to_string(),
        })?;

    Ok(Response::OK(GetDeploymentResponse { data: deployment }))
}
