use aether_auth::Identity;
use aether_core::platform::{PlatformRights, ports::PlatformService};
use axum::{Extension, extract::State};
use axum_extra::routing::TypedPath;
use serde::Serialize;
use utoipa::ToSchema;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath)]
#[typed_path("/platform/rights")]
pub struct MyRightsRoute;

#[derive(Serialize, ToSchema, PartialEq)]
pub struct MyRightsResponse {
    /// Empty for somebody who operates nothing here, which is most people.
    pub data: PlatformRights,
}

#[utoipa::path(
    get,
    path = "/rights",
    summary = "what the caller may do to this installation",
    tag = "platform",
    description = "Answers for the caller and nobody else, so it needs no right of its own: \
                   asking what you hold is not a way to learn anything you do not.",
    responses(
        (status = 200, description = "The rights the caller holds, possibly none", body = MyRightsResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn my_rights_handler(
    _: MyRightsRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<MyRightsResponse>, ApiError> {
    let rights = state.service.my_platform_rights(identity).await?;

    Ok(Response::OK(MyRightsResponse { data: rights }))
}
