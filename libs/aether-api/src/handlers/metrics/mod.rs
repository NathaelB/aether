use axum::{Router, middleware::from_fn_with_state};
use axum_extra::routing::RouterExt;
use utoipa::OpenApi;

use crate::{
    handlers::metrics::{
        get_active_users::{__path_get_active_users_handler, get_active_users_handler},
        get_deployment_usage::{__path_get_deployment_usage_handler, get_deployment_usage_handler},
        report_usage_metrics::{__path_report_usage_metrics_handler, report_usage_metrics_handler},
    },
    router::service_auth_middleware,
    state::AppState,
};

pub mod get_active_users;
pub mod get_deployment_usage;
pub mod report_usage_metrics;

/// The console's two read endpoints, organisation-scoped like every other
/// domain nested under `/organisations`.
#[derive(OpenApi)]
#[openapi(
    paths(get_deployment_usage_handler, get_active_users_handler),
    tags(
        (name = "metrics", description = "Usage metrics read by the console.")
    )
)]
pub struct MetricsApiDoc;

/// Herald's one write endpoint, kept apart from [`MetricsApiDoc`] because it
/// nests under `/deployments` rather than `/organisations` -- Herald reports
/// against a deployment it already knows, not an organisation it looks up.
#[derive(OpenApi)]
#[openapi(
    paths(report_usage_metrics_handler),
    tags(
        (name = "metrics", description = "Usage metrics reported by data planes.")
    )
)]
pub struct MetricsIngestApiDoc;

pub fn metrics_routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .typed_get(get_deployment_usage_handler)
        .typed_get(get_active_users_handler)
        .typed_post(report_usage_metrics_handler)
        .layer(from_fn_with_state(
            app_state.clone(),
            service_auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::metrics_routes;
    use crate::test_helpers::app_state;

    #[tokio::test]
    async fn metrics_routes_builds() {
        let state = app_state();
        let _router = metrics_routes(state);
    }
}
