use std::str::FromStr;

use aether_auth::Identity;
use aether_core::{
    organisation::{OrganisationId, value_objects::OrganisationStatus},
    platform::{Tenant, TenantQuery, ports::PlatformService},
};
use axum::{
    Extension,
    extract::{Query, State},
};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath)]
#[typed_path("/platform/organisations")]
pub struct TenantsRoute;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TenantsQuery {
    /// `active`, `suspended` or whichever statuses the platform uses.
    pub status: Option<String>,
    pub limit: Option<usize>,

    /// The last organisation of the previous page.
    pub cursor: Option<Uuid>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct TenantsResponse {
    pub data: Vec<Tenant>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<OrganisationId>,
}

#[utoipa::path(
    get,
    path = "/organisations",
    summary = "list every organisation on the installation",
    tag = "platform",
    description = "With how many deployments and members each holds. Somebody asking which \
                   organisations they belong to is a different question, answered by \
                   /users/@me/organisations.",
    params(TenantsQuery),
    responses(
        (status = 200, description = "One page of the tenants", body = TenantsResponse),
        (status = 400, description = "The request does not describe a page", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "Running the installation is a separate right", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_tenants_handler(
    _: TenantsRoute,
    Query(query): Query<TenantsQuery>,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<TenantsResponse>, ApiError> {
    let page = state
        .service
        .list_tenants(identity, query.into_query()?)
        .await?;

    Ok(Response::OK(TenantsResponse {
        data: page.tenants,
        next_cursor: page.next_cursor,
    }))
}

impl TenantsQuery {
    fn into_query(self) -> Result<TenantQuery, ApiError> {
        let refused = |reason: String| ApiError::BadRequest { reason };

        let status = self
            .status
            .as_deref()
            .map(OrganisationStatus::from_str)
            .transpose()
            .map_err(|_| {
                refused(format!(
                    "'{}' is not an organisation status",
                    self.status.clone().unwrap_or_default()
                ))
            })?;

        Ok(
            TenantQuery::new(self.limit, self.cursor.map(OrganisationId))
                .map_err(refused)?
                .with_status(status),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query() -> TenantsQuery {
        TenantsQuery {
            status: None,
            limit: None,
            cursor: None,
        }
    }

    /// A status nobody uses would match no row, and an empty page reads as an
    /// installation with no tenants rather than as a typo.
    #[test]
    fn a_status_nobody_uses_is_refused_rather_than_matched_against_nothing() {
        let refused = TenantsQuery {
            status: Some("dormant".to_string()),
            ..query()
        }
        .into_query()
        .expect_err("a status nobody uses was accepted");

        let ApiError::BadRequest { reason } = refused else {
            panic!("the refusal was not a bad request");
        };
        assert!(reason.contains("dormant"), "got {reason}");
    }

    #[test]
    fn a_page_beyond_the_ceiling_is_refused() {
        assert!(
            TenantsQuery {
                limit: Some(aether_core::platform::MAX_PAGE + 1),
                ..query()
            }
            .into_query()
            .is_err()
        );
    }

    #[test]
    fn a_request_with_no_filter_asks_about_every_tenant() {
        assert!(
            query()
                .into_query()
                .expect("a valid request")
                .status
                .is_none()
        );
    }
}
