use axum::{Router, middleware::from_fn_with_state};
use axum_extra::routing::RouterExt;
use utoipa::OpenApi;

use crate::{
    handlers::invitations::{
        accept::accept_invitation_handler,
        invitations::{
            __path_invite_handler, __path_list_invitations_handler,
            __path_revoke_invitation_handler, invite_handler, list_invitations_handler,
            revoke_invitation_handler,
        },
    },
    router::service_auth_middleware,
    state::AppState,
};

pub mod accept;
#[allow(clippy::module_inception)]
pub mod invitations;

#[derive(OpenApi)]
#[openapi(
    paths(
        list_invitations_handler,
        invite_handler,
        revoke_invitation_handler,
    ),
    tags(
        (name = "invitations", description = "How somebody who has no account yet gets into an organisation.")
    )
)]
pub struct InvitationApiDoc;

// accept_invitation_handler is documented on the root ApiDoc instead: its
// path sits under no prefix, and nest() has no way to say that.

pub fn invitation_routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .typed_get(list_invitations_handler)
        .typed_post(invite_handler)
        .typed_delete(revoke_invitation_handler)
        .typed_post(accept_invitation_handler)
        .layer(from_fn_with_state(
            app_state.clone(),
            service_auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::invitation_routes;
    use crate::test_helpers::app_state;

    #[tokio::test]
    async fn invitation_routes_builds() {
        let state = app_state();
        let _router = invitation_routes(state);
    }
}
