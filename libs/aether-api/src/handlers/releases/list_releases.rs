use aether_auth::Identity;
use aether_core::{
    catalog::{Release, ReleaseInUse, ports::ReleaseService},
    deployments::DeploymentKind,
};
use axum::{Extension, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ListReleasesResponse {
    pub data: Vec<Release>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ListReleasesInUseResponse {
    pub data: Vec<ReleaseInUse>,
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/releases/{kind}")]
pub struct ListReleasesRoute {
    pub kind: DeploymentKind,
}

#[utoipa::path(
    get,
    path = "/{kind}",
    summary = "list published releases",
    tag = "releases",
    description = "List the releases of a product that customers may see. A release that has only been planned is never returned here.",
    params(ListReleasesRoute),
    responses(
        (status = 200, description = "Published releases, newest first", body = ListReleasesResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_releases_handler(
    ListReleasesRoute { kind }: ListReleasesRoute,
    State(state): State<AppState>,
) -> Result<Response<ListReleasesResponse>, ApiError> {
    let releases = state.service.list_published_releases(kind).await?;

    Ok(Response::OK(ListReleasesResponse { data: releases }))
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/operator/releases/{kind}")]
pub struct ListReleasesForOperatorRoute {
    pub kind: DeploymentKind,
}

#[utoipa::path(
    get,
    path = "/operator/{kind}",
    summary = "list every release",
    tag = "releases",
    description = "List every release of a product, planning included. Operator only.",
    params(ListReleasesForOperatorRoute),
    responses(
        (status = 200, description = "Every release, newest first, with how many deployments run it", body = ListReleasesInUseResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "Forbidden", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_releases_for_operator_handler(
    ListReleasesForOperatorRoute { kind }: ListReleasesForOperatorRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<ListReleasesInUseResponse>, ApiError> {
    let releases = state
        .service
        .list_releases_for_operator(identity, kind)
        .await?;

    Ok(Response::OK(ListReleasesInUseResponse { data: releases }))
}
