use aether_auth::Identity;
use aether_core::{
    backups::ports::BackupService,
    deployments::{Deployment, DeploymentId, cutover::CutoverCommand},
    organisation::OrganisationId,
    user::UserId,
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments/{deployment_id}/cutover")]
pub struct CutoverRoute {
    pub organisation_id: Uuid,

    /// Ends up serving the hostname `demote` currently does.
    pub deployment_id: Uuid,
}

#[derive(Deserialize, ToSchema)]
pub struct CutoverRequest {
    /// Ends up on the hostname `deployment_id` currently gives up.
    pub demote: Uuid,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct CutoverResponseData {
    pub promoted: Deployment,
    pub demoted: Deployment,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct CutoverResponse {
    data: CutoverResponseData,
}

#[utoipa::path(
    post,
    path = "/{organisation_id}/deployments/{deployment_id}/cutover",
    summary = "move which deployment serves a hostname",
    tag = "deployments",
    description = "Trades names between two deployments related by a restore. The one named \
                   in the path ends up serving the hostname the other currently does; the \
                   other keeps running, renamed to what the first gave up. Symmetric: calling \
                   this again with the two reversed is a cutback, not a second operation.",
    params(CutoverRoute),
    request_body = CutoverRequest,
    responses(
        (status = 200, description = "The two deployments, after the swap", body = CutoverResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not cut over this organisation's deployments", body = ApiError),
        (status = 404, description = "No such deployment", body = ApiError),
        (status = 409, description = "The two are not related by a restore, or one is deleted", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn cutover_handler(
    CutoverRoute {
        organisation_id,
        deployment_id,
    }: CutoverRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<CutoverRequest>,
) -> Result<Response<CutoverResponse>, ApiError> {
    let requested_by =
        identity
            .id()
            .parse::<UserId>()
            .map_err(|e| ApiError::InternalServerError {
                reason: e.to_string(),
            })?;

    let (promoted, demoted) = state
        .service
        .cutover(
            identity,
            CutoverCommand {
                organisation_id: OrganisationId(organisation_id),
                promote: DeploymentId(deployment_id),
                demote: DeploymentId(request.demote),
                requested_by,
            },
        )
        .await?;

    Ok(Response::OK(CutoverResponse {
        data: CutoverResponseData { promoted, demoted },
    }))
}
