use aether_auth::Identity;
use aether_core::{
    offers::{OfferAvailability, ports::OfferService},
    organisation::OrganisationId,
};
use axum::{Extension, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/offers")]
pub struct OffersRoute {
    pub organisation_id: Uuid,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ListOffersResponse {
    pub data: Vec<OfferAvailability>,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/offers",
    summary = "list the offers an organisation may choose from",
    tag = "organisations",
    description = "The whole catalogue, each entry saying whether this organisation's tier \
                   opens it and, when it does not, the tier that would. Hiding a closed \
                   offer makes an upsell invisible; showing it with what opens it turns a \
                   refusal into a next step.",
    params(OffersRoute),
    responses(
        (status = 200, description = "What this organisation may choose from", body = ListOffersResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller is not in this organisation", body = ApiError),
        (status = 404, description = "No such organisation", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_offers_handler(
    OffersRoute { organisation_id }: OffersRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<ListOffersResponse>, ApiError> {
    let offers = state
        .service
        .list_offers(identity, OrganisationId(organisation_id))
        .await?;

    Ok(Response::OK(ListOffersResponse { data: offers }))
}
