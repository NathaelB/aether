use aether_auth::Identity;
use aether_core::{
    dataplane::{ports::DataPlaneService, value_objects::DataPlaneId},
    deployments::{DeploymentId, commands::ReportDeploymentOutcomeCommand},
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/dataplanes/{dataplane_id}/deployments/{deployment_id}/outcome")]
pub struct ReportOutcomeRoute {
    pub dataplane_id: DataPlaneId,
    pub deployment_id: DeploymentId,
}

#[derive(Deserialize, ToSchema)]
pub struct ReportOutcomeRequest {
    /// What the data plane observed: `running`, `failed` or `deleted`.
    ///
    /// All three are things a component saw rather than inferred -- the
    /// operator wrote `Running`/`ready` or `Failed` onto the resource, and
    /// deletion is reported by whatever removed it. None of them is a timeout,
    /// which is why none needs a policy about when to give up.
    pub outcome: String,

    /// The version the data plane sees running, when it reports one.
    ///
    /// Optional because it is new: a data plane built before this field sends
    /// nothing, and refusing its report would have it retry something that can
    /// never succeed. It is what separates an upgrade that landed from one
    /// that has not started, since an instance answers on its old version
    /// until the rollout replaces it.
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ReportOutcomeResponseData {
    /// `false` when the report changed nothing -- an unknown deployment, or one
    /// whose state does not accept this outcome.
    ///
    /// Not an error: reports are at-least-once, so a redelivered one is
    /// expected and must not be answered with a failure that makes a data plane
    /// retry something already recorded.
    pub recorded: bool,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ReportOutcomeResponse {
    data: ReportOutcomeResponseData,
}

#[utoipa::path(
    post,
    path = "/{dataplane_id}/deployments/{deployment_id}/outcome",
    summary = "report what a data plane did with a deployment",
    tag = "dataplanes",
    description = "Records an outcome the data plane observed directly. This is the only \
                   path by which the control plane learns what happened to a deployment \
                   after it was handed over.",
    params(ReportOutcomeRoute),
    request_body = ReportOutcomeRequest,
    responses(
        (status = 200, description = "Outcome recorded", body = ReportOutcomeResponse),
        (status = 400, description = "Unknown outcome", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "Caller is not herald", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn report_outcome_handler(
    ReportOutcomeRoute {
        dataplane_id,
        deployment_id,
    }: ReportOutcomeRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<ReportOutcomeRequest>,
) -> Result<Response<ReportOutcomeResponse>, ApiError> {
    let command = ReportDeploymentOutcomeCommand::parse(
        dataplane_id,
        deployment_id,
        request.outcome.as_str(),
        request.version.as_deref(),
    )
    .map_err(|reason| ApiError::BadRequest { reason })?;

    let recorded = state.service.report_outcome(identity, command).await?;

    Ok(Response::OK(ReportOutcomeResponse {
        data: ReportOutcomeResponseData { recorded },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An outcome the control plane does not know is a caller mistake, not a
    /// deployment that failed. Rejecting it stops a typo being recorded as
    /// something that happened.
    #[test]
    fn an_unknown_outcome_is_rejected() {
        let result = ReportDeploymentOutcomeCommand::parse(
            DataPlaneId(uuid::Uuid::new_v4()),
            DeploymentId(uuid::Uuid::new_v4()),
            "exploded",
            None,
        );

        assert!(result.is_err());
    }

    #[test]
    fn every_outcome_the_data_plane_can_report_is_accepted() {
        for outcome in ["deleted", "running", "failed"] {
            assert!(
                ReportDeploymentOutcomeCommand::parse(
                    DataPlaneId(uuid::Uuid::new_v4()),
                    DeploymentId(uuid::Uuid::new_v4()),
                    outcome,
                    None,
                )
                .is_ok(),
                "{outcome}"
            );
        }
    }
}
