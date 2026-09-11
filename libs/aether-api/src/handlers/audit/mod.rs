use axum::{Router, middleware::from_fn_with_state};
use axum_extra::routing::RouterExt;
use utoipa::OpenApi;

use crate::{
    handlers::audit::list_audit_log::{__path_list_audit_log_handler, list_audit_log_handler},
    router::service_auth_middleware,
    state::AppState,
};

pub mod list_audit_log;

#[derive(OpenApi)]
#[openapi(
    paths(list_audit_log_handler),
    tags(
        (name = "audit", description = "Audit trail endpoints scoped to organisations.")
    )
)]
pub struct AuditApiDoc;

pub fn audit_routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .typed_get(list_audit_log_handler)
        .layer(from_fn_with_state(
            app_state.clone(),
            service_auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::audit_routes;
    use crate::test_helpers::app_state;

    #[tokio::test]
    async fn audit_routes_builds() {
        let state = app_state();
        let _router = audit_routes(state);
    }
}
