use aether_auth::Identity;
use aether_core::{
    dataplane::{ports::DataPlaneService, value_objects::DataPlaneId},
    deployments::Deployment,
};
use axum::{Extension, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ListDeploymentsForDataPlaneResponse {
    pub data: Vec<Deployment>,
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/dataplanes/{dataplane_id}/deployments")]
pub struct ListDeploymentsForDataPlaneRoute {
    pub dataplane_id: DataPlaneId,
}

#[derive(Deserialize, IntoParams)]
pub struct ListDeploymentsForDataPlaneQueryParams {
    pub shard_index: Option<usize>,
    pub shard_count: Option<usize>,
    pub limit: Option<usize>,
    pub cursor: Option<String>,
}

#[utoipa::path(
    get,
    path = "/{dataplane_id}/deployments",
    summary = "list deployments for dataplane",
    tag = "dataplanes",
    description = "List deployments associated with the specified dataplane.",
    params(ListDeploymentsForDataPlaneQueryParams),
    responses(
        (status = 200, description = "List of deployments for dataplane", body = ListDeploymentsForDataPlaneResponse),
        (status = 400, description = "Invalid dataplane id", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    )
)]
pub async fn list_deployments_for_dataplane_handler(
    ListDeploymentsForDataPlaneRoute { dataplane_id }: ListDeploymentsForDataPlaneRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<ListDeploymentsForDataPlaneResponse>, ApiError> {
    let deployments = state
        .service
        .get_deployments_in_dataplane(identity, dataplane_id)
        .await?;

    Ok(Response::OK(ListDeploymentsForDataPlaneResponse {
        data: deployments,
    }))
}
