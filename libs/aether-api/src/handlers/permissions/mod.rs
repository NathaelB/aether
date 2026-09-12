use axum::{Router, middleware::from_fn_with_state};
use axum_extra::routing::RouterExt;
use utoipa::OpenApi;

use crate::{
    handlers::permissions::my_permissions::{
        __path_my_permissions_handler, my_permissions_handler,
    },
    router::service_auth_middleware,
    state::AppState,
};

pub mod my_permissions;

#[derive(OpenApi)]
#[openapi(
    paths(my_permissions_handler),
    tags(
        (name = "permissions", description = "What the signed-in caller may do in an organisation.")
    )
)]
pub struct PermissionsApiDoc;

pub fn permissions_routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .typed_get(my_permissions_handler)
        .layer(from_fn_with_state(
            app_state.clone(),
            service_auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::permissions_routes;
    use crate::test_helpers::app_state;

    #[tokio::test]
    async fn permissions_routes_builds() {
        let state = app_state();
        let _router = permissions_routes(state);
    }
}
