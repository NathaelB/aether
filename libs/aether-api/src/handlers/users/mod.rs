use axum::{Router, middleware::from_fn_with_state};
use axum_extra::routing::RouterExt;
use utoipa::OpenApi;

use crate::{
    handlers::users::get_user_organisations::{
        __path_get_user_organisations_handler, get_user_organisations_handler,
    },
    router::service_auth_middleware,
    state::AppState,
};

pub mod get_user_organisations;

#[derive(OpenApi)]
#[openapi(
    paths(
        get_user_organisations_handler,
    ),
    tags(
        (name = "users", description = "User management endpoints.")
    )
)]
pub struct UserApiDoc;

pub fn user_routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .typed_get(get_user_organisations_handler)
        .layer(from_fn_with_state(
            app_state.clone(),
            service_auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::user_routes;
    use crate::test_helpers::app_state;

    #[tokio::test]
    async fn user_routes_builds() {
        let state = app_state();
        let _router = user_routes(state);
    }
}
