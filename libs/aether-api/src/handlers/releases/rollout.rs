use std::str::FromStr;

use aether_auth::Identity;
use aether_core::{
    catalog::{
        HeldBackDataPlane, Release, ReleaseAvailability, Rollout, RolloutCoverage,
        RolloutPercentage,
        commands::{RolloutCoveragePreview, WidenRolloutCommand},
        ports::ReleaseService,
    },
    deployments::{DeploymentId, DeploymentKind},
    organisation::{OrganisationId, value_objects::Plan},
    version::Version,
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{errors::ApiError, response::Response, state::AppState};

/// What an operator sends to widen a rollout, or to preview one before it is
/// saved. Raw strings rather than domain types, parsed and validated in this
/// handler the same way `version()` already does for a release version --
/// `Plan` and `RolloutPercentage` are not `Deserialize` themselves, so a
/// payload cannot bypass validation by constructing them directly.
#[derive(Debug, Deserialize, ToSchema)]
pub struct RolloutRequest {
    /// 0 to 100.
    pub percentage: u8,
    /// Absent means every plan. A named list restricts the percentage to
    /// those plans; see the module docs on [`Rollout`] for what widening and
    /// narrowing mean here.
    #[serde(default)]
    pub plans: Option<Vec<String>>,
    /// Organisations that get the release however small the percentage or
    /// however the plans are restricted.
    #[serde(default)]
    pub pilot_organisations: Vec<uuid::Uuid>,
}

fn rollout(request: RolloutRequest) -> Result<Rollout, ApiError> {
    let percentage =
        RolloutPercentage::new(request.percentage).map_err(|e| ApiError::BadRequest {
            reason: e.to_string(),
        })?;
    let plans = request
        .plans
        .map(|raw| {
            raw.iter()
                .map(|value| {
                    Plan::from_str(value).map_err(|e| ApiError::BadRequest {
                        reason: e.to_string(),
                    })
                })
                .collect::<Result<Vec<Plan>, ApiError>>()
        })
        .transpose()?;

    // Built directly rather than by widening from `Rollout::full()`: this is
    // the candidate the operator asked for, not a change applied on top of
    // one already in force. Comparing it against what a release currently
    // holds is `Release::widen_rollout`'s job, once this reaches the service.
    let pilot_organisations = request
        .pilot_organisations
        .into_iter()
        .map(OrganisationId)
        .collect();

    Ok(Rollout::new(percentage, plans, pilot_organisations))
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ReleaseResponse {
    pub data: Release,
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/operator/releases/{kind}/{version}/rollout")]
pub struct WidenRolloutRoute {
    pub kind: DeploymentKind,
    pub version: String,
}

fn version(raw: &str) -> Result<Version, ApiError> {
    Version::parse(raw).map_err(|e| ApiError::BadRequest {
        reason: e.to_string(),
    })
}

#[utoipa::path(
    put,
    path = "/operator/{kind}/{version}/rollout",
    summary = "widen a release's rollout",
    tag = "releases",
    description = "Widens the percentage, plan targeting or pilot organisations a release is \
                   offered to. Never narrows: a percentage, a plan or a pilot organisation, \
                   once part of a rollout, cannot be taken back. Operator only.",
    params(WidenRolloutRoute),
    request_body = RolloutRequest,
    responses(
        (status = 200, description = "The release with its widened rollout", body = ReleaseResponse),
        (status = 400, description = "Not a percentage, an unknown plan, or a narrowing", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "Forbidden", body = ApiError),
        (status = 404, description = "Not in the catalogue", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn widen_rollout_handler(
    WidenRolloutRoute { kind, version: raw }: WidenRolloutRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<RolloutRequest>,
) -> Result<Response<ReleaseResponse>, ApiError> {
    let release = state
        .service
        .widen_rollout(
            identity,
            WidenRolloutCommand {
                kind,
                version: version(&raw)?,
                rollout: rollout(request)?,
            },
        )
        .await?;

    Ok(Response::OK(ReleaseResponse { data: release }))
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/operator/releases/{kind}/{version}/rollout/preview")]
pub struct PreviewRolloutRoute {
    pub kind: DeploymentKind,
    pub version: String,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct RolloutCoverageResponse {
    pub data: RolloutCoverage,
}

#[utoipa::path(
    post,
    path = "/operator/{kind}/{version}/rollout/preview",
    summary = "preview how many deployments a candidate rollout would cover",
    tag = "releases",
    description = "Answers how many deployments a rollout would be offered to without saving \
                   it. Operator only.",
    params(PreviewRolloutRoute),
    request_body = RolloutRequest,
    responses(
        (status = 200, description = "How many deployments this candidate rollout covers", body = RolloutCoverageResponse),
        (status = 400, description = "Not a percentage or an unknown plan", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "Forbidden", body = ApiError),
        (status = 404, description = "Not in the catalogue", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn preview_rollout_coverage_handler(
    PreviewRolloutRoute { kind, version: raw }: PreviewRolloutRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<RolloutRequest>,
) -> Result<Response<RolloutCoverageResponse>, ApiError> {
    let coverage = state
        .service
        .preview_rollout_coverage(
            identity,
            RolloutCoveragePreview {
                kind,
                version: version(&raw)?,
                rollout: rollout(request)?,
            },
        )
        .await?;

    Ok(Response::OK(RolloutCoverageResponse { data: coverage }))
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/operator/releases/{kind}/{version}/hold-backs")]
pub struct ReleaseHoldBacksRoute {
    pub kind: DeploymentKind,
    pub version: String,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ReleaseHoldBacksResponse {
    pub data: Vec<HeldBackDataPlane>,
}

#[utoipa::path(
    get,
    path = "/operator/{kind}/{version}/hold-backs",
    summary = "which data planes hold a release back",
    tag = "releases",
    description = "Names the data planes whose operator/chart version is behind what this \
                   release requires. Empty when the release has no requirement or nothing is \
                   behind it. Operator only.",
    params(ReleaseHoldBacksRoute),
    responses(
        (status = 200, description = "Data planes holding this release back", body = ReleaseHoldBacksResponse),
        (status = 400, description = "The version is not a semver", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "Forbidden", body = ApiError),
        (status = 404, description = "Not in the catalogue", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn release_hold_backs_handler(
    ReleaseHoldBacksRoute { kind, version: raw }: ReleaseHoldBacksRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<ReleaseHoldBacksResponse>, ApiError> {
    let held_back = state
        .service
        .release_hold_backs(identity, kind, version(&raw)?)
        .await?;

    Ok(Response::OK(ReleaseHoldBacksResponse { data: held_back }))
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/releases/deployments/{organisation_id}/{deployment_id}")]
pub struct ReleaseAvailabilityRoute {
    pub organisation_id: OrganisationId,
    pub deployment_id: DeploymentId,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ReleaseAvailabilityResponse {
    pub data: Vec<ReleaseAvailability>,
}

#[utoipa::path(
    get,
    path = "/deployments/{organisation_id}/{deployment_id}",
    summary = "releases as one deployment sees them",
    tag = "releases",
    description = "Every release of this deployment's product, with a reason attached to every \
                   one it may not install -- an unpublished status, an operator requirement its \
                   data plane does not meet, or a rollout it falls outside of. A deployment \
                   belonging to another organisation is reported as not found.",
    params(ReleaseAvailabilityRoute),
    responses(
        (status = 200, description = "Every release this deployment may see, with why", body = ReleaseAvailabilityResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 404, description = "No such deployment for this organisation", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn release_availability_handler(
    ReleaseAvailabilityRoute {
        organisation_id,
        deployment_id,
    }: ReleaseAvailabilityRoute,
    State(state): State<AppState>,
) -> Result<Response<ReleaseAvailabilityResponse>, ApiError> {
    let availability = state
        .service
        .release_availability_for_deployment(organisation_id, deployment_id)
        .await?;

    Ok(Response::OK(ReleaseAvailabilityResponse {
        data: availability,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_percentage_over_a_hundred_is_a_bad_request() {
        let error = rollout(RolloutRequest {
            percentage: 142,
            plans: None,
            pilot_organisations: Vec::new(),
        })
        .expect_err("not a percentage");

        assert!(matches!(error, ApiError::BadRequest { .. }));
    }

    #[test]
    fn an_unknown_plan_is_a_bad_request() {
        let error = rollout(RolloutRequest {
            percentage: 10,
            plans: Some(vec!["gold".to_string()]),
            pilot_organisations: Vec::new(),
        })
        .expect_err("gold is not a plan");

        assert!(matches!(error, ApiError::BadRequest { .. }));
    }

    #[test]
    fn a_well_formed_request_becomes_a_rollout() {
        let rollout = rollout(RolloutRequest {
            percentage: 25,
            plans: Some(vec!["business".to_string(), "enterprise".to_string()]),
            pilot_organisations: vec![uuid::Uuid::new_v4()],
        })
        .expect("well formed");

        assert_eq!(rollout.percentage(), RolloutPercentage::new(25).unwrap());
        assert_eq!(rollout.plans().map(<[Plan]>::len), Some(2));
        assert_eq!(rollout.pilot_organisations().len(), 1);
    }
}
