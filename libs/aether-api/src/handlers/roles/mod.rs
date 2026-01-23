use axum::{Router, middleware::from_fn_with_state};
use axum_extra::routing::RouterExt;
use utoipa::OpenApi;

use crate::{
    handlers::roles::{
        create_role::{__path_create_role_handler, create_role_handler},
        delete_role::{__path_delete_role_handler, delete_role_handler},
        get_role::{__path_get_role_handler, get_role_handler},
        list_roles::{__path_list_roles_handler, list_roles_handler},
        update_role::{__path_update_role_handler, update_role_handler},
    },
    router::service_auth_middleware,
    state::AppState,
};

pub mod create_role;
pub mod delete_role;
pub mod get_role;
pub mod list_roles;
pub mod update_role;

#[derive(OpenApi)]
#[openapi(
    paths(
        list_roles_handler,
        create_role_handler,
        get_role_handler,
        update_role_handler,
        delete_role_handler,
    ),
    tags(
        (name = "roles", description = "Role management endpoints scoped to organisations.")
    )
)]
pub struct RoleApiDoc;

pub fn role_routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .typed_get(list_roles_handler)
        .typed_post(create_role_handler)
        .typed_get(get_role_handler)
        .typed_patch(update_role_handler)
        .typed_delete(delete_role_handler)
        .layer(from_fn_with_state(
            app_state.clone(),
            service_auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::role_routes;
    use crate::test_helpers::app_state;

    #[tokio::test]
    async fn role_routes_builds() {
        let state = app_state();
        let _router = role_routes(state);
    }
}
