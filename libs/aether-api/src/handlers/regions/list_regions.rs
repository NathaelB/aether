use aether_auth::Identity;
use aether_core::dataplane::{ports::DataPlaneService, value_objects::Region};
use axum::{Extension, extract::State};
use axum_extra::routing::TypedPath;
use serde::Serialize;
use utoipa::ToSchema;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ListRegionsResponse {
    pub data: Vec<Region>,
}

#[derive(TypedPath)]
#[typed_path("/regions")]
pub struct ListRegionsRoute;

#[utoipa::path(
    get,
    path = "",
    summary = "list the regions this installation serves",
    tag = "regions",
    description = "Where a deployment can be created. Says nothing about the infrastructure \
                   behind a region -- how many clusters serve it, who owns them, or how full \
                   they are -- which is what makes it safe to answer for any caller.",
    responses(
        (status = 200, description = "Regions", body = ListRegionsResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_regions_handler(
    _: ListRegionsRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<ListRegionsResponse>, ApiError> {
    let regions = state.service.list_regions(identity).await?;

    Ok(Response::OK(ListRegionsResponse { data: regions }))
}
