use axum::{Router, middleware::from_fn_with_state};
use axum_extra::routing::RouterExt;
use utoipa::OpenApi;

use crate::{
    handlers::organisations::{
        create_organisation::{__path_create_organisation_handler, create_organisation_handler},
        get_organisations::{__path_get_organisations_handler, get_organisations_handler},
    },
    router::service_auth_middleware,
    state::AppState,
};

pub mod create_organisation;
pub mod get_organisations;

#[derive(OpenApi)]
#[openapi(
    paths(get_organisations_handler, create_organisation_handler),
    tags(
        (name = "organisation", description = "Organisation management endpoints.")
    )
)]
pub struct OrganisationApiDoc;

pub fn organisation_routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .typed_get(get_organisations_handler)
        .typed_post(create_organisation_handler)
        .layer(from_fn_with_state(
            app_state.clone(),
            service_auth_middleware,
        ))
}
