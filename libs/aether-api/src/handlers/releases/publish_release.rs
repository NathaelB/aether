use aether_auth::Identity;
use aether_core::{
    catalog::{
        BreakingRisk, Release, ReleaseNotes, ReleaseStatus,
        commands::{AnnounceReleaseCommand, MoveReleaseCommand, ReviseReleaseCommand},
        ports::ReleaseService,
    },
    deployments::DeploymentKind,
    version::Version,
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Deserialize, ToSchema)]
pub struct PublishReleaseRequest {
    pub version: String,
    pub risk: BreakingRisk,
    #[serde(default)]
    pub notes: String,
    /// Versions to pass through on the way to this one. Empty (the default)
    /// means it can be reached directly.
    #[serde(default)]
    pub steps_through: Vec<String>,
    /// Lowest operator/chart version a data plane must run to host this
    /// release. Absent means any operator may install it.
    #[serde(default)]
    pub minimum_operator_version: Option<String>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ReleaseResponse {
    pub data: Release,
}

fn version(raw: &str) -> Result<Version, ApiError> {
    Version::parse(raw).map_err(|e| ApiError::BadRequest {
        reason: e.to_string(),
    })
}

fn versions(raw: &[String]) -> Result<Vec<Version>, ApiError> {
    raw.iter().map(|value| version(value)).collect()
}

fn optional_version(raw: Option<&str>) -> Result<Option<Version>, ApiError> {
    raw.map(version).transpose()
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/releases/operator/{kind}")]
pub struct PublishReleaseRoute {
    pub kind: DeploymentKind,
}

#[utoipa::path(
    post,
    path = "/operator/{kind}",
    summary = "announce a release",
    tag = "releases",
    description = "Record a version of a product. It starts unpublished and is not offered to anyone until it is moved to available. Operator only.",
    params(PublishReleaseRoute),
    request_body = PublishReleaseRequest,
    responses(
        (status = 200, description = "The release as recorded", body = ReleaseResponse),
        (status = 400, description = "The version is not a semver", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "Forbidden", body = ApiError),
        (status = 409, description = "The catalogue already holds it", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn publish_release_handler(
    PublishReleaseRoute { kind }: PublishReleaseRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<PublishReleaseRequest>,
) -> Result<Response<ReleaseResponse>, ApiError> {
    let release = state
        .service
        .publish_release(
            identity,
            AnnounceReleaseCommand {
                kind,
                version: version(&request.version)?,
                risk: request.risk,
                notes: ReleaseNotes(request.notes),
                steps_through: versions(&request.steps_through)?,
                minimum_operator_version: optional_version(
                    request.minimum_operator_version.as_deref(),
                )?,
            },
        )
        .await?;

    Ok(Response::OK(ReleaseResponse { data: release }))
}

#[derive(Deserialize, ToSchema)]
pub struct ReviseReleaseRequest {
    pub risk: BreakingRisk,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub steps_through: Vec<String>,
    #[serde(default)]
    pub minimum_operator_version: Option<String>,
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/releases/operator/{kind}/{version}")]
pub struct ReviseReleaseRoute {
    pub kind: DeploymentKind,
    pub version: String,
}

#[utoipa::path(
    patch,
    path = "/operator/{kind}/{version}",
    summary = "revise the notes and risk of a release",
    tag = "releases",
    description = "What a version breaks is usually found out after it ships. Operator only.",
    params(ReviseReleaseRoute),
    request_body = ReviseReleaseRequest,
    responses(
        (status = 200, description = "The revised release", body = ReleaseResponse),
        (status = 400, description = "The version is not a semver", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "Forbidden", body = ApiError),
        (status = 404, description = "Not in the catalogue", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn revise_release_handler(
    ReviseReleaseRoute { kind, version: raw }: ReviseReleaseRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<ReviseReleaseRequest>,
) -> Result<Response<ReleaseResponse>, ApiError> {
    let release = state
        .service
        .revise_release(
            identity,
            ReviseReleaseCommand {
                kind,
                version: version(&raw)?,
                risk: request.risk,
                notes: ReleaseNotes(request.notes),
                steps_through: versions(&request.steps_through)?,
                minimum_operator_version: optional_version(
                    request.minimum_operator_version.as_deref(),
                )?,
            },
        )
        .await?;

    Ok(Response::OK(ReleaseResponse { data: release }))
}

#[derive(Deserialize, ToSchema)]
pub struct MoveReleaseRequest {
    pub status: ReleaseStatus,
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/releases/operator/{kind}/{version}/status")]
pub struct MoveReleaseRoute {
    pub kind: DeploymentKind,
    pub version: String,
}

#[utoipa::path(
    put,
    path = "/operator/{kind}/{version}/status",
    summary = "move a release forward",
    tag = "releases",
    description = "A release only ever moves forward. Withdrawing stops new installs and upgrades; it does not touch what already runs the version. Operator only.",
    params(MoveReleaseRoute),
    request_body = MoveReleaseRequest,
    responses(
        (status = 200, description = "The release at its new status", body = ReleaseResponse),
        (status = 400, description = "The version is not a semver", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "Forbidden", body = ApiError),
        (status = 404, description = "Not in the catalogue", body = ApiError),
        (status = 409, description = "A release never moves backwards", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn move_release_handler(
    MoveReleaseRoute { kind, version: raw }: MoveReleaseRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<MoveReleaseRequest>,
) -> Result<Response<ReleaseResponse>, ApiError> {
    let release = state
        .service
        .move_release(
            identity,
            MoveReleaseCommand {
                kind,
                version: version(&raw)?,
                status: request.status,
            },
        )
        .await?;

    Ok(Response::OK(ReleaseResponse { data: release }))
}
