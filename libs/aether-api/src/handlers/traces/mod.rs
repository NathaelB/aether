use axum::{Router, middleware::from_fn_with_state};
use axum_extra::routing::RouterExt;
use utoipa::OpenApi;

use crate::{
    handlers::traces::{
        get_trace::{__path_get_trace_handler, get_trace_handler},
        search_traces::{__path_search_traces_handler, search_traces_handler},
    },
    router::service_auth_middleware,
    state::AppState,
};

pub mod get_trace;
pub mod search_traces;

#[derive(OpenApi)]
#[openapi(
    paths(search_traces_handler, get_trace_handler),
    tags(
        (name = "traces", description = "OpenTelemetry trace search endpoints scoped to organisations.")
    )
)]
pub struct TracesApiDoc;

pub fn traces_routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .typed_get(search_traces_handler)
        .typed_get(get_trace_handler)
        .layer(from_fn_with_state(
            app_state.clone(),
            service_auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::traces_routes;
    use crate::test_helpers::app_state;

    #[tokio::test]
    async fn traces_routes_builds() {
        let state = app_state();
        let _router = traces_routes(state);
    }
}
