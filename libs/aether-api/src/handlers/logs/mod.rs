use axum::{Router, middleware::from_fn_with_state};
use axum_extra::routing::RouterExt;
use utoipa::OpenApi;

use crate::{
    handlers::logs::{
        group_logs::{__path_group_logs_handler, group_logs_handler},
        search_logs::{__path_search_logs_handler, search_logs_handler},
    },
    router::service_auth_middleware,
    state::AppState,
};

pub mod group_logs;
pub mod search_logs;

#[derive(OpenApi)]
#[openapi(
    paths(search_logs_handler, group_logs_handler),
    tags(
        (name = "logs", description = "Log search endpoints scoped to organisations.")
    )
)]
pub struct LogsApiDoc;

pub fn logs_routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .typed_get(search_logs_handler)
        .typed_get(group_logs_handler)
        .layer(from_fn_with_state(
            app_state.clone(),
            service_auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::logs_routes;
    use crate::test_helpers::app_state;

    #[tokio::test]
    async fn logs_routes_builds() {
        let state = app_state();
        let _router = logs_routes(state);
    }
}
