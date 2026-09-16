use aether_auth::Identity;
use aether_core::{
    organisation::{OrganisationId, value_objects::Plan},
    platform::{Tenant, ports::TenantPlanService},
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/platform/organisations/{organisation_id}/plan")]
pub struct TenantPlanRoute {
    pub organisation_id: Uuid,
}

#[derive(Deserialize, ToSchema)]
pub struct MoveTenantPlanRequest {
    /// `free`, `starter`, `business` or `enterprise`.
    ///
    /// The offers this opens are not named here. Which plan opens which offer
    /// is stated by the offers themselves, in one direction, so a request that
    /// could carry both could contradict itself.
    pub plan: String,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct TenantResponse {
    pub data: Tenant,
}

#[utoipa::path(
    put,
    path = "/organisations/{organisation_id}/plan",
    summary = "move an organisation to another plan",
    tag = "platform",
    description = "What an organisation may buy follows from the plan it is on. A plan whose \
                   limits sit below what the organisation already holds is refused rather than \
                   applied, because applying it leaves a tenant that can create nothing and \
                   cannot come back under the line without deleting something in use.",
    params(TenantPlanRoute),
    request_body = MoveTenantPlanRequest,
    responses(
        (status = 200, description = "The organisation, on its new plan", body = TenantResponse),
        (status = 400, description = "No such plan", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "Acting on a tenant is a separate right", body = ApiError),
        (status = 404, description = "No such organisation", body = ApiError),
        (status = 409, description = "The plan sits below what the organisation already holds", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn move_tenant_plan_handler(
    TenantPlanRoute { organisation_id }: TenantPlanRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<MoveTenantPlanRequest>,
) -> Result<Response<TenantResponse>, ApiError> {
    let plan = request
        .plan
        .parse::<Plan>()
        .map_err(|error| ApiError::BadRequest {
            reason: error.to_string(),
        })?;

    let tenant = state
        .service
        .move_tenant_to_plan(identity, OrganisationId(organisation_id), plan)
        .await?;

    Ok(Response::OK(TenantResponse { data: tenant }))
}
