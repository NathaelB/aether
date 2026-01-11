use aether_core::deployments::{Deployment, ports::DeploymentService};
use axum::extract::State;
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ListDeploymentsResponse {
    data: Vec<Deployment>,
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments")]
pub struct ListDeploymentsRoute {
    pub organisation_id: Uuid,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/deployments",
    summary = "list deployments",
    tag = "deployments",
    description = "List deployments for the specified organisation.",
    params(ListDeploymentsRoute),
    responses(
        (status = 200, description = "List of deployments", body = ListDeploymentsResponse),
        (status = 400, description = "Invalid organisation id", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    )
)]
pub async fn list_deployments_handler(
    ListDeploymentsRoute { organisation_id }: ListDeploymentsRoute,
    State(state): State<AppState>,
) -> Result<Response<ListDeploymentsResponse>, ApiError> {
    let organisation_id = organisation_id.into();

    let deployments = state
        .service
        .list_deployments_by_organisation(organisation_id)
        .await?;

    Ok(Response::OK(ListDeploymentsResponse { data: deployments }))
}
