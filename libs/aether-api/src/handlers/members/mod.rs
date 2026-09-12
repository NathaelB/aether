use axum::{Router, middleware::from_fn_with_state};
use axum_extra::routing::RouterExt;
use utoipa::OpenApi;

use crate::{
    handlers::members::{
        get_member::{
            __path_get_member_handler, __path_remove_member_handler, get_member_handler,
            remove_member_handler,
        },
        list_members::{__path_list_members_handler, list_members_handler},
        set_member_roles::{__path_set_member_roles_handler, set_member_roles_handler},
    },
    router::service_auth_middleware,
    state::AppState,
};

pub mod get_member;
pub mod list_members;
pub mod set_member_roles;

#[derive(OpenApi)]
#[openapi(
    paths(
        list_members_handler,
        get_member_handler,
        remove_member_handler,
        set_member_roles_handler,
    ),
    tags(
        (name = "members", description = "Who is in an organisation, and what they may do there.")
    )
)]
pub struct MemberApiDoc;

pub fn member_routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .typed_get(list_members_handler)
        .typed_get(get_member_handler)
        .typed_delete(remove_member_handler)
        .typed_put(set_member_roles_handler)
        .layer(from_fn_with_state(
            app_state.clone(),
            service_auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::member_routes;
    use crate::test_helpers::app_state;

    #[tokio::test]
    async fn member_routes_builds() {
        let state = app_state();
        let _router = member_routes(state);
    }
}
