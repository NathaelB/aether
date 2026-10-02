use aether_auth::Identity;
use aether_core::organisation::{
    OrganisationId,
    features::{FeatureService, PlanFeatures},
};
use axum::{Extension, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/features")]
pub struct FeaturesRoute {
    pub organisation_id: Uuid,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ListFeaturesResponse {
    pub data: PlanFeatures,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/features",
    summary = "list the IAM features an organisation's plan opens",
    tag = "organisations",
    description = "The whole set, each entry saying whether this organisation's plan opens it \
                   and, when it does not, the cheapest plan that would.",
    params(FeaturesRoute),
    responses(
        (status = 200, description = "What this organisation's plan opens", body = ListFeaturesResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller is not in this organisation", body = ApiError),
        (status = 404, description = "No such organisation", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_features_handler(
    FeaturesRoute { organisation_id }: FeaturesRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<ListFeaturesResponse>, ApiError> {
    let features = state
        .service
        .list_features(identity, OrganisationId(organisation_id))
        .await?;

    Ok(Response::OK(ListFeaturesResponse { data: features }))
}
