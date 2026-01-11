use aether_core::deployments::ports::DeploymentService;
use axum::extract::State;
use axum_extra::routing::TypedPath;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments/{deployment_id}")]
pub struct DeleteDeploymentRoute {
    pub organisation_id: Uuid,
    pub deployment_id: Uuid,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct DeleteDeploymentResponse {
    success: bool,
}

#[utoipa::path(
    delete,
    path = "/{organisation_id}/deployments/{deployment_id}",
    summary = "delete deployment",
    tag = "deployments",
    description = "Delete a deployment within the specified organisation.",
    params(DeleteDeploymentRoute),
    responses(
        (status = 200, description = "Deployment deleted successfully", body = DeleteDeploymentResponse),
        (status = 400, description = "Deployment not found", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn delete_deployment_handler(
    DeleteDeploymentRoute {
        organisation_id,
        deployment_id,
    }: DeleteDeploymentRoute,
    State(state): State<AppState>,
) -> Result<Response<DeleteDeploymentResponse>, ApiError> {
    let organisation_id = organisation_id.into();
    let deployment_id = deployment_id.into();

    state
        .service
        .delete_deployment_for_organisation(organisation_id, deployment_id)
        .await
        .map_err(|_| ApiError::BadRequest {
            reason: "deployment not found".to_string(),
        })?;

    Ok(Response::OK(DeleteDeploymentResponse { success: true }))
}
