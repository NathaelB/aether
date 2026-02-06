use aether_auth::Identity;
use aether_core::dataplane::{entities::DataPlane, ports::DataPlaneService};
use axum::{Extension, extract::State};
use axum_extra::routing::TypedPath;
use serde::Serialize;
use utoipa::ToSchema;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ListDataplanesResponse {
    pub data: Vec<DataPlane>,
}

#[derive(TypedPath)]
#[typed_path("/dataplanes")]
pub struct ListDataplanesRoute;

#[utoipa::path(
    get,
    path = "",
    summary = "list dataplanes",
    tag = "dataplanes",
    description = "List all dataplanes accessible to the authenticated user.",
    responses(
        (status = 200, description = "List of dataplanes", body = ListDataplanesResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_dataplanes_handler(
    _: ListDataplanesRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<ListDataplanesResponse>, ApiError> {
    let dataplanes = state.service.list_dataplanes(identity).await?;

    Ok(Response::OK(ListDataplanesResponse { data: dataplanes }))
}
