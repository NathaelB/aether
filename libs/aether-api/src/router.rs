use axum::{
    Router,
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::get,
};
use tower_http::trace::TraceLayer;
use tracing::info_span;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    auth_middleware,
    errors::ApiError,
    handlers::{
        actions::action_routes, deployments::deployment_routes, organisations::organisation_routes,
        roles::role_routes, users::user_routes,
    },
    openapi::ApiDoc,
    state::AppState,
};

pub async fn service_auth_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    match auth_middleware(State(state.service), req, next).await {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

async fn handler() -> &'static str {
    "Aether API is running"
}

pub fn router(state: AppState) -> Result<Router, ApiError> {
    let trace_layer = TraceLayer::new_for_http().make_span_with(|request: &Request| {
        let uri: String = request.uri().to_string();
        info_span!("http_request", method = ?request.method(), uri)
    });

    let openapi = ApiDoc::openapi();

    let router = Router::new()
        .route("/", get(handler))
        .merge(Scalar::with_url("/scalar", openapi.clone()))
        .merge(SwaggerUi::new("/swagger").url("/api-docs/openapi.json", openapi.clone()))
        .merge(organisation_routes(state.clone()))
        .merge(role_routes(state.clone()))
        .merge(deployment_routes(state.clone()))
        .merge(action_routes(state.clone()))
        .merge(user_routes(state.clone()))
        .layer(trace_layer)
        .with_state(state);

    Ok(router)
}

#[cfg(test)]
mod tests {
    use super::router;
    use crate::test_helpers::app_state;

    #[tokio::test]
    async fn router_builds() {
        let state = app_state();
        let result = router(state);
        assert!(result.is_ok());
    }
}
