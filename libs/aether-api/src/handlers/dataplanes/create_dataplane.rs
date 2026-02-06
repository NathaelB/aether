use aether_auth::Identity;
use aether_core::dataplane::{
    entities::DataPlane,
    ports::DataPlaneService,
    value_objects::{Capacity, CreateDataplaneCommand, DataPlaneMode, Region},
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath)]
#[typed_path("/dataplanes")]
pub struct CreateDataPlaneRoute;

#[derive(Deserialize, ToSchema)]
pub struct CreateDataPlaneRequest {
    pub mode: DataPlaneMode,
    pub region: Region,
    pub capacity: Capacity,
}

#[utoipa::path(
    post,
    path = "",
    summary = "create dataplane",
    tag = "dataplanes",
    request_body = CreateDataPlaneRequest,
    description = "Create a new dataplane with the specified configuration.",
    responses(
        (status = 200, description = "Created dataplane", body = DataPlane),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    )
)]
pub async fn create_dataplane_handler(
    _: CreateDataPlaneRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<CreateDataPlaneRequest>,
) -> Result<Response<DataPlane>, ApiError> {
    let dataplane = state
        .service
        .create_dataplane(
            identity,
            CreateDataplaneCommand {
                capacity: request.capacity,
                mode: request.mode,
                region: request.region,
            },
        )
        .await?;

    Ok(Response::Created(dataplane))
}
