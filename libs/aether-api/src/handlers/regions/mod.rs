use axum::{Router, middleware::from_fn_with_state};
use axum_extra::routing::RouterExt;
use utoipa::OpenApi;

use crate::handlers::regions::list_regions::{__path_list_regions_handler, list_regions_handler};
use crate::{router::service_auth_middleware, state::AppState};

pub mod list_regions;

#[derive(OpenApi)]
#[openapi(paths(list_regions_handler))]
pub struct RegionApiDoc;

pub fn regions_routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .typed_get(list_regions_handler)
        .layer(from_fn_with_state(
            app_state.clone(),
            service_auth_middleware,
        ))
}
