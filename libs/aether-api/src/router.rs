use axum::{
    Router,
    extract::{Request, State},
    http::{
        HeaderValue, Method,
        header::{ACCEPT, AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, LOCATION},
    },
    middleware::Next,
    response::{IntoResponse, Response},
    routing::get,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info_span;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};
use utoipa_swagger_ui::SwaggerUi;

use crate::{
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
    service_auth(State(state.service), req, |req| next.run(req)).await
}

async fn service_auth<T, F, Fut>(State(state): State<T>, req: Request, next: F) -> Response
where
    T: aether_core::auth::ports::AuthService
        + aether_core::user::ports::UserService
        + Clone
        + Send
        + Sync
        + 'static,
    F: FnOnce(Request) -> Fut,
    Fut: std::future::Future<Output = Response> + Send,
{
    match crate::auth_middleware_impl(State(state), req, next).await {
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

    let allowed_origins: Vec<HeaderValue> = vec![HeaderValue::from_static("http://localhost:5173")];

    let cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::DELETE,
            Method::PUT,
            Method::PATCH,
            Method::OPTIONS,
        ])
        .allow_origin(allowed_origins)
        .allow_headers([
            AUTHORIZATION,
            CONTENT_TYPE,
            CONTENT_LENGTH,
            ACCEPT,
            LOCATION,
        ])
        .allow_credentials(true);

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
        .layer(cors)
        .with_state(state);

    Ok(router)
}

#[cfg(test)]
mod tests {
    use super::handler;
    use super::router;
    use super::service_auth;
    use crate::test_helpers::app_state;
    use aether_auth::{Identity, User};
    use aether_core::CoreError;
    use aether_core::auth::ports::AuthService;
    use aether_core::user::{commands::CreateUserCommand, ports::UserService};
    use axum::body::Body;
    use axum::extract::{Request, State};
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use std::future::Future;

    #[tokio::test]
    async fn router_builds() {
        let state = app_state();
        let result = router(state);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn handler_returns_ok_message() {
        let message = handler().await;
        assert_eq!(message, "Aether API is running");
    }

    #[derive(Clone)]
    struct FakeAuthService {
        fail: bool,
    }

    impl AuthService for FakeAuthService {
        fn get_identity(
            &self,
            _token: &str,
        ) -> impl Future<Output = Result<Identity, CoreError>> + Send {
            let fail = self.fail;
            Box::pin(async move {
                if fail {
                    Err(CoreError::InvalidIdentity)
                } else {
                    Ok(Identity::User(User {
                        id: "user-1".to_string(),
                        username: "user".to_string(),
                        email: None,
                        name: None,
                        roles: vec![],
                    }))
                }
            })
        }
    }

    impl UserService for FakeAuthService {
        fn create_user(
            &self,
            _command: CreateUserCommand,
        ) -> impl Future<Output = Result<aether_core::user::User, CoreError>> + Send {
            Box::pin(async move {
                Err(CoreError::DatabaseError {
                    message: "not implemented".to_string(),
                })
            })
        }
    }

    #[tokio::test]
    async fn service_auth_returns_unauthorized_on_missing_header() {
        let state = FakeAuthService { fail: false };
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();

        let response = service_auth(State(state), req, |_req| async move {
            StatusCode::OK.into_response()
        })
        .await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn service_auth_passes_through_on_success() {
        let state = FakeAuthService { fail: false };
        let req = Request::builder()
            .uri("/")
            .header(axum::http::header::AUTHORIZATION, "Bearer token")
            .body(Body::empty())
            .unwrap();

        let response = service_auth(State(state), req, |_req| async move {
            StatusCode::OK.into_response()
        })
        .await;

        assert_eq!(response.status(), StatusCode::OK);
    }
}
