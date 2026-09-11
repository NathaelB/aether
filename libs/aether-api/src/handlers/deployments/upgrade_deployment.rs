use aether_auth::Identity;
use aether_core::{
    deployments::{Deployment, DeploymentId},
    organisation::OrganisationId,
    upgrades::{commands::RequestUpgradeCommand, ports::UpgradeService},
    version::Version,
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments/{deployment_id}/upgrade")]
pub struct UpgradeDeploymentRoute {
    pub organisation_id: Uuid,
    pub deployment_id: Uuid,
}

#[derive(Deserialize, ToSchema)]
pub struct UpgradeDeploymentRequest {
    pub version: String,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct UpgradeDeploymentResponse {
    pub data: Deployment,
    /// Whether this is a patch, a minor or a major. Carried back because the
    /// caller has just been told the upgrade started and this is what says how
    /// much to worry.
    pub change: String,
}

#[utoipa::path(
    post,
    path = "/{organisation_id}/deployments/{deployment_id}/upgrade",
    summary = "upgrade a deployment",
    tag = "deployments",
    description = "Move a deployment to another version. The target must be in the catalogue, installable, and ahead of what the deployment runs. The deployment must be settled: one that is still coming up, already upgrading or being torn down is refused.",
    params(UpgradeDeploymentRoute),
    request_body = UpgradeDeploymentRequest,
    responses(
        (status = 200, description = "The upgrade was accepted and handed to the data plane", body = UpgradeDeploymentResponse),
        (status = 400, description = "The version is not a semver", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not upgrade this organisation's deployments", body = ApiError),
        (status = 404, description = "No such deployment, or no such release", body = ApiError),
        (status = 409, description = "The deployment is not settled, or the target cannot be installed", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn upgrade_deployment_handler(
    UpgradeDeploymentRoute {
        organisation_id,
        deployment_id,
    }: UpgradeDeploymentRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<UpgradeDeploymentRequest>,
) -> Result<Response<UpgradeDeploymentResponse>, ApiError> {
    let target = Version::parse(&request.version).map_err(|e| ApiError::BadRequest {
        reason: e.to_string(),
    })?;

    let accepted = state
        .service
        .request_upgrade(
            identity,
            RequestUpgradeCommand {
                organisation_id: OrganisationId(organisation_id),
                deployment_id: DeploymentId(deployment_id),
                target,
            },
        )
        .await?;

    Ok(Response::OK(UpgradeDeploymentResponse {
        data: accepted.deployment,
        change: format!("{:?}", accepted.change).to_lowercase(),
    }))
}
