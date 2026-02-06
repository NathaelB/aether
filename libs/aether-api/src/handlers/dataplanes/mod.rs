use axum::{Router, middleware::from_fn_with_state};
use axum_extra::routing::RouterExt;
use utoipa::OpenApi;

use crate::handlers::dataplanes::{
    claim_actions::{__path_claim_actions_handler, claim_actions_handler},
    create_dataplane::{__path_create_dataplane_handler, create_dataplane_handler},
    get_dataplane::{__path_get_dataplane_handler, get_dataplane_handler},
    list_dataplanes::{__path_list_dataplanes_handler, list_dataplanes_handler},
    list_deployments_for_dataplane::{
        __path_list_deployments_for_dataplane_handler, list_deployments_for_dataplane_handler,
    },
};
use crate::{router::service_auth_middleware, state::AppState};

pub mod claim_actions;
pub mod create_dataplane;
pub mod get_dataplane;
pub mod list_dataplanes;
pub mod list_deployments_for_dataplane;

#[derive(OpenApi)]
#[openapi(paths(
    list_dataplanes_handler,
    get_dataplane_handler,
    list_deployments_for_dataplane_handler,
    claim_actions_handler,
    create_dataplane_handler
))]
pub struct DataPlaneApiDoc;

pub fn dataplanes_routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .typed_get(list_dataplanes_handler)
        .typed_post(create_dataplane_handler)
        .typed_get(get_dataplane_handler)
        .typed_get(list_deployments_for_dataplane_handler)
        .typed_post(claim_actions_handler)
        .layer(from_fn_with_state(
            app_state.clone(),
            service_auth_middleware,
        ))
}
