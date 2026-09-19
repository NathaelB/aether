use aether_auth::Identity;
use aether_core::{
    backups::{commands::RecordDrillOutcomeCommand, ports::BackupService},
    dataplane::value_objects::DataPlaneId,
    deployments::DeploymentId,
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/dataplanes/{dataplane_id}/deployments/{deployment_id}/drill")]
pub struct ReportDrillOutcomeRoute {
    pub dataplane_id: DataPlaneId,
    pub deployment_id: DeploymentId,
}

#[derive(Deserialize, ToSchema)]
pub struct ReportDrillOutcomeRequest {
    /// `drill_succeeded` or `drill_failed`.
    pub outcome: String,

    /// How long the drill took. Required on success -- the restore time
    /// objective this feature exists to measure -- and optional on a
    /// failure, which can happen before there was anything to time.
    #[serde(default)]
    pub duration_seconds: Option<u64>,

    /// Why a drill failed. Required on failure, and what a failed drill's
    /// audit entry is written from.
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ReportDrillOutcomeResponseData {}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ReportDrillOutcomeResponse {
    data: ReportDrillOutcomeResponseData,
}

#[utoipa::path(
    post,
    path = "/{dataplane_id}/deployments/{deployment_id}/drill",
    summary = "report what a restore drill proved, or did not",
    tag = "dataplanes",
    description = "Records the result of a drill (#185): a success lands on the deployment as \
                   last_verified_restore_at and how long it took; a failure is written to the \
                   audit trail and changes nothing else.",
    params(ReportDrillOutcomeRoute),
    request_body = ReportDrillOutcomeRequest,
    responses(
        (status = 200, description = "Outcome recorded", body = ReportDrillOutcomeResponse),
        (status = 400, description = "Unknown outcome, or missing duration/reason", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "Caller is not herald, or speaks for another data plane", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn report_drill_outcome_handler(
    ReportDrillOutcomeRoute {
        dataplane_id,
        deployment_id,
    }: ReportDrillOutcomeRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<ReportDrillOutcomeRequest>,
) -> Result<Response<ReportDrillOutcomeResponse>, ApiError> {
    let command = RecordDrillOutcomeCommand::parse(
        dataplane_id,
        deployment_id,
        request.outcome.as_str(),
        request.duration_seconds,
        request.reason,
        chrono::Utc::now(),
    )
    .map_err(|reason| ApiError::BadRequest { reason })?;

    state
        .service
        .record_drill_outcome(identity, command)
        .await?;

    Ok(Response::OK(ReportDrillOutcomeResponse {
        data: ReportDrillOutcomeResponseData {},
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An outcome the control plane does not know is a caller mistake, not a
    /// drill result. Rejecting it stops a typo being recorded as something
    /// that happened.
    #[test]
    fn an_unknown_outcome_is_rejected() {
        let result = RecordDrillOutcomeCommand::parse(
            DataPlaneId(uuid::Uuid::new_v4()),
            DeploymentId(uuid::Uuid::new_v4()),
            "exploded",
            None,
            None,
            chrono::Utc::now(),
        );

        assert!(result.is_err());
    }

    #[test]
    fn a_success_without_a_duration_is_rejected() {
        let result = RecordDrillOutcomeCommand::parse(
            DataPlaneId(uuid::Uuid::new_v4()),
            DeploymentId(uuid::Uuid::new_v4()),
            "drill_succeeded",
            None,
            None,
            chrono::Utc::now(),
        );

        assert!(result.is_err());
    }

    #[test]
    fn a_failure_without_a_reason_is_rejected() {
        let result = RecordDrillOutcomeCommand::parse(
            DataPlaneId(uuid::Uuid::new_v4()),
            DeploymentId(uuid::Uuid::new_v4()),
            "drill_failed",
            Some(30),
            None,
            chrono::Utc::now(),
        );

        assert!(result.is_err());
    }

    #[test]
    fn both_outcomes_the_data_plane_can_report_are_accepted() {
        assert!(
            RecordDrillOutcomeCommand::parse(
                DataPlaneId(uuid::Uuid::new_v4()),
                DeploymentId(uuid::Uuid::new_v4()),
                "drill_succeeded",
                Some(42),
                None,
                chrono::Utc::now(),
            )
            .is_ok()
        );
        assert!(
            RecordDrillOutcomeCommand::parse(
                DataPlaneId(uuid::Uuid::new_v4()),
                DeploymentId(uuid::Uuid::new_v4()),
                "drill_failed",
                Some(42),
                Some("the archive was corrupted".to_string()),
                chrono::Utc::now(),
            )
            .is_ok()
        );
    }
}
