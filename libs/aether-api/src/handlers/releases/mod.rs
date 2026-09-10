use axum::{Router, middleware::from_fn_with_state};
use axum_extra::routing::RouterExt;
use utoipa::OpenApi;

use crate::{router::service_auth_middleware, state::AppState};

pub use self::{
    list_releases::{
        __path_list_releases_for_operator_handler, __path_list_releases_handler,
        list_releases_for_operator_handler, list_releases_handler,
    },
    publish_release::{
        __path_move_release_handler, __path_publish_release_handler, __path_revise_release_handler,
        move_release_handler, publish_release_handler, revise_release_handler,
    },
};

pub mod list_releases;
pub mod publish_release;

#[derive(OpenApi)]
#[openapi(paths(
    list_releases_handler,
    list_releases_for_operator_handler,
    publish_release_handler,
    revise_release_handler,
    move_release_handler
))]
pub struct ReleaseApiDoc;

pub fn releases_routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .typed_get(list_releases_handler)
        .typed_get(list_releases_for_operator_handler)
        .typed_post(publish_release_handler)
        .typed_patch(revise_release_handler)
        .typed_put(move_release_handler)
        .layer(from_fn_with_state(
            app_state.clone(),
            service_auth_middleware,
        ))
}
