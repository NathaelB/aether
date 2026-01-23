use std::str::FromStr;

use aether_core::organisation::{
    Organisation, ports::OrganisationService, value_objects::OrganisationStatus,
};
use axum::extract::{Query, State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{errors::ApiError, handlers::default_limit, response::Response, state::AppState};

#[derive(Serialize, ToSchema, PartialEq)]
pub struct GetOrganisationsResponse {
    data: Vec<Organisation>,
}

#[derive(Deserialize, IntoParams)]
pub struct GetOrganisationsQuery {
    status: Option<String>,

    #[serde(default = "default_limit")]
    limit: usize,

    #[serde(default)]
    offset: usize,
}

#[derive(TypedPath)]
#[typed_path("/organisations")]
pub struct GetOrganisationsRoute;

#[utoipa::path(
    get,
    path = "",
    summary = "get organisations",
    tag = "organisation",
    description = "Retrieve a list of all organisations.",
    params(GetOrganisationsQuery),
    responses(
        (status = 200, description = "List of Organisations", body = GetOrganisationsResponse),
        (status = 400, description = "Invalid query parameters", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    )
)]
pub async fn get_organisations_handler(
    _: GetOrganisationsRoute,
    Query(query): Query<GetOrganisationsQuery>,
    State(state): State<AppState>,
) -> Result<Response<GetOrganisationsResponse>, ApiError> {
    let status =
        match query.status {
            Some(status_str) => Some(OrganisationStatus::from_str(&status_str).map_err(|_| {
                ApiError::BadRequest {
                    reason: format!("Invalid status value: {}", status_str),
                }
            })?),
            None => None,
        };

    let organisations = state
        .service
        .get_organisations(status, query.limit, query.offset)
        .await?;

    Ok(Response::OK(GetOrganisationsResponse {
        data: organisations,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::app_state;

    #[tokio::test]
    async fn get_organisations_rejects_invalid_status() {
        let state = app_state();
        let query = GetOrganisationsQuery {
            status: Some("not-a-status".to_string()),
            limit: 10,
            offset: 0,
        };

        let result =
            get_organisations_handler(GetOrganisationsRoute, Query(query), State(state)).await;

        assert!(matches!(result, Err(ApiError::BadRequest { .. })));
    }
}
