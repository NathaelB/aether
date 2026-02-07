use aether_auth::Identity;
use aether_core::dataplane::{
    entities::DataPlane, ports::DataPlaneService, value_objects::DataPlaneId,
};
use axum::{Extension, extract::State};
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, Deserialize)]
#[typed_path("/dataplanes/{dataplane_id}")]
pub struct GetDataPlaneRoute {
    pub dataplane_id: DataPlaneId,
}

#[utoipa::path(
    get,
    path = "/{dataplane_id}",
    summary = "get dataplane",
    tag = "dataplanes",
    description = "Get details of the specified dataplane.",
    responses(
        (status = 200, description = "Dataplane details", body = DataPlane),
        (status = 400, description = "Invalid dataplane id", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    )
)]
pub async fn get_dataplane_handler(
    GetDataPlaneRoute { dataplane_id }: GetDataPlaneRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<DataPlane>, ApiError> {
    let dataplane = state.service.get_dataplane(identity, dataplane_id).await?;

    Ok(Response::OK(dataplane))
}
