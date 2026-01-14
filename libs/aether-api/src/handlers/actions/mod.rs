use axum::{Router, middleware::from_fn_with_state};
use axum_extra::routing::RouterExt;
use utoipa::OpenApi;

use crate::{
    handlers::actions::{
        get_action::{__path_get_action_handler, get_action_handler},
        list_actions::{__path_list_actions_handler, list_actions_handler},
    },
    router::service_auth_middleware,
    state::AppState,
};

pub mod get_action;
pub mod list_actions;

#[derive(OpenApi)]
#[openapi(
    paths(
        list_actions_handler,
        get_action_handler,
    ),
    tags(
        (name = "actions", description = "Action endpoints scoped to deployments.")
    )
)]
pub struct ActionApiDoc;

pub fn action_routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .typed_get(list_actions_handler)
        .typed_get(get_action_handler)
        .layer(from_fn_with_state(
            app_state.clone(),
            service_auth_middleware,
        ))
}
