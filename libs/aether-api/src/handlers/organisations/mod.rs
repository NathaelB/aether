use axum::{Router, middleware::from_fn_with_state};
use axum_extra::routing::RouterExt;
use utoipa::OpenApi;

use crate::{
    handlers::organisations::{
        create_organisation::{__path_create_organisation_handler, create_organisation_handler},
        list_offers::{__path_list_offers_handler, list_offers_handler},
    },
    router::service_auth_middleware,
    state::AppState,
};

pub mod create_organisation;
pub mod list_offers;

#[derive(OpenApi)]
#[openapi(
    paths(create_organisation_handler, list_offers_handler),
    tags(
        (name = "organisation", description = "Organisation management endpoints.")
    )
)]
pub struct OrganisationApiDoc;

/// The routes themselves, before anything is layered on them.
///
/// Split out so a test can ask the router what it serves. Through the
/// middleware every request without a token is a 401, which cannot tell a
/// route that is gone from one that is merely closed -- and the test below is
/// about a route being gone.
fn routes() -> Router<AppState> {
    Router::new()
        .typed_post(create_organisation_handler)
        .typed_get(list_offers_handler)
}

pub fn organisation_routes(app_state: AppState) -> Router<AppState> {
    routes().layer(from_fn_with_state(
        app_state.clone(),
        service_auth_middleware,
    ))
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    use super::organisation_routes;
    use crate::test_helpers::app_state;

    #[tokio::test]
    async fn organisation_routes_builds() {
        let state = app_state();
        let _router = organisation_routes(state);
    }

    /// `GET /organisations` listed every organisation on the installation to
    /// anybody holding a token: authenticated, and authorised by nothing. It
    /// was removed rather than gated, because nothing consumed it -- the
    /// platform asks through `/platform/organisations`, which is gated, and a
    /// member asks through `/users/@me/organisations`, which is scoped to
    /// them.
    ///
    /// A 405 rather than a 404: `POST /organisations` still creates one, so
    /// the path exists and only the method is gone. If somebody re-adds the
    /// listing, this is what says so.
    #[tokio::test]
    async fn nothing_lists_every_organisation_to_whoever_asks() {
        let router = super::routes().with_state(app_state());

        let answer = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/organisations")
                    .body(Body::empty())
                    .expect("a request"),
            )
            .await
            .expect("the router answered");

        assert_eq!(
            answer.status(),
            axum::http::StatusCode::METHOD_NOT_ALLOWED,
            "the installation-wide organisation listing is served again"
        );
    }
}
