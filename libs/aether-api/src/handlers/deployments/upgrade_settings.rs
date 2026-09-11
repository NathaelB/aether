use aether_auth::Identity;
use aether_core::{
    deployments::{Deployment, DeploymentId},
    organisation::OrganisationId,
    upgrades::{
        commands::SetUpgradeSettingsCommand,
        policy::{AutoUpgradePolicy, MaintenanceWindow},
        ports::UpgradeService,
    },
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments/{deployment_id}/upgrade-settings")]
pub struct UpgradeSettingsRoute {
    pub organisation_id: Uuid,
    pub deployment_id: Uuid,
}

#[derive(Deserialize, ToSchema)]
pub struct MaintenanceWindowRequest {
    /// `mon` through `sun`.
    pub day: String,
    /// Local wall clock, `HH:MM`.
    pub start: String,
    pub minutes: i64,
    /// An IANA zone such as `Europe/Paris`. A fixed offset would drift by an
    /// hour twice a year against the time the customer actually wrote down.
    pub timezone: String,
}

#[derive(Deserialize, ToSchema)]
pub struct SetUpgradeSettingsRequest {
    pub auto_upgrade: AutoUpgradePolicy,
    /// Absent means no window, and therefore no automatic upgrade whatever the
    /// policy says. Declining to name one is a choice, not an omission.
    #[serde(default)]
    pub maintenance_window: Option<MaintenanceWindowRequest>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct UpgradeSettingsResponse {
    pub data: Deployment,
}

fn parse_window(request: MaintenanceWindowRequest) -> Result<MaintenanceWindow, ApiError> {
    let day = request
        .day
        .parse::<chrono::Weekday>()
        .map_err(|_| ApiError::BadRequest {
            reason: format!("'{}' is not a day, expected mon through sun", request.day),
        })?;

    let start = chrono::NaiveTime::parse_from_str(&request.start, "%H:%M").map_err(|_| {
        ApiError::BadRequest {
            reason: format!("'{}' is not a time of day, expected HH:MM", request.start),
        }
    })?;

    let timezone = request
        .timezone
        .parse::<chrono_tz::Tz>()
        .map_err(|_| ApiError::BadRequest {
            reason: format!(
                "'{}' is not a time zone, expected an IANA name such as Europe/Paris",
                request.timezone
            ),
        })?;

    MaintenanceWindow::new(
        day,
        start,
        chrono::Duration::minutes(request.minutes),
        timezone,
    )
    .map_err(|e| ApiError::BadRequest {
        reason: e.to_string(),
    })
}

#[utoipa::path(
    put,
    path = "/{organisation_id}/deployments/{deployment_id}/upgrade-settings",
    summary = "set what the platform may upgrade on its own, and when",
    tag = "deployments",
    description = "A major is never applied automatically, whatever the policy says. Without a maintenance window nothing is applied automatically either, whatever the policy says.",
    params(UpgradeSettingsRoute),
    request_body = SetUpgradeSettingsRequest,
    responses(
        (status = 200, description = "The deployment with its new settings", body = UpgradeSettingsResponse),
        (status = 400, description = "The window cannot be read, or cannot close", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not change this deployment", body = ApiError),
        (status = 404, description = "No such deployment", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn set_upgrade_settings_handler(
    UpgradeSettingsRoute {
        organisation_id,
        deployment_id,
    }: UpgradeSettingsRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<SetUpgradeSettingsRequest>,
) -> Result<Response<UpgradeSettingsResponse>, ApiError> {
    let maintenance_window = request.maintenance_window.map(parse_window).transpose()?;

    let deployment = state
        .service
        .set_upgrade_settings(
            identity,
            SetUpgradeSettingsCommand {
                organisation_id: OrganisationId(organisation_id),
                deployment_id: DeploymentId(deployment_id),
                auto_upgrade: request.auto_upgrade,
                maintenance_window,
            },
        )
        .await?;

    Ok(Response::OK(UpgradeSettingsResponse { data: deployment }))
}
