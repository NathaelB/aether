use aether_auth::Identity;
use aether_core::deployments::ports::DeploymentService;
use aether_core::{
    deployments::DeploymentId,
    organisation::OrganisationId,
    upgrades::{ports::UpgradeService, run::InFlightUpgrade},
};
use axum::extract::{Extension, State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments/{deployment_id}/upgrade")]
pub struct UpgradeInFlightRoute {
    pub organisation_id: Uuid,
    pub deployment_id: Uuid,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct UpgradeInFlightResponse {
    /// Absent when nothing is being applied, which is the ordinary state.
    pub data: Option<InFlightUpgrade>,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/deployments/{deployment_id}/upgrade",
    summary = "read the upgrade being applied right now",
    tag = "deployments",
    description = "The whole planned path, and the version the deployment reports running, so a screen can say which step is under way.",
    params(UpgradeInFlightRoute),
    responses(
        (status = 200, description = "The upgrade under way, or nothing", body = UpgradeInFlightResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 404, description = "No such deployment", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn upgrade_in_flight_handler(
    UpgradeInFlightRoute {
        organisation_id,
        deployment_id,
    }: UpgradeInFlightRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<UpgradeInFlightResponse>, ApiError> {
    // Through the scoped read first, which gates on VIEW_INSTANCES and
    // answers not-found for a deployment of another organisation. Whether an
    // upgrade is running is a fact about an instance, and reading it is
    // reading the instance.
    state
        .service
        .get_deployment_for_organisation(
            identity,
            OrganisationId(organisation_id),
            DeploymentId(deployment_id),
        )
        .await?;

    let in_flight = state
        .service
        .upgrade_in_flight(OrganisationId(organisation_id), DeploymentId(deployment_id))
        .await?;

    Ok(Response::OK(UpgradeInFlightResponse { data: in_flight }))
}
