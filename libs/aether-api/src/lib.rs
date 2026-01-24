use std::net::{SocketAddr, ToSocketAddrs};

use aether_auth::Token;
use aether_core::{CoreError, auth::ports::AuthService};
use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::{IntoResponse, Response},
};

use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use crate::{
    args::LogArgs,
    errors::{ApiError, ApiErrorResponse},
};

pub mod args;
pub mod auth;
pub mod errors;
pub mod handlers;
pub mod openapi;
pub mod response;
pub mod router;
pub mod state;

#[cfg(test)]
pub(crate) mod test_helpers {
    use std::{sync::Arc, time::Duration};

    use aether_auth::{Identity, User};
    use aether_core::AetherService;
    use sqlx::postgres::PgPoolOptions;

    use crate::{args, state::AppState};

    pub fn app_state() -> AppState {
        let pool = PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(50))
            .connect_lazy("postgres://user:pass@127.0.0.1:1/db")
            .expect("valid database url");

        AppState {
            args: Arc::new(args::Args {
                log: args::LogArgs::default(),
                db: args::DatabaseArgs::default(),
                auth: args::AuthArgs {
                    issuer: "http://localhost:8888/realms/aether".to_string(),
                },
                server: args::ServerArgs::default(),
            }),
            service: AetherService::new(pool),
        }
    }

    pub fn user_identity(id: &str) -> Identity {
        Identity::User(User {
            id: id.to_string(),
            username: "user".to_string(),
            email: None,
            name: None,
            roles: vec![],
        })
    }
}

pub fn init_logger(args: &LogArgs) {
    let filter = EnvFilter::try_new(&args.filter).unwrap_or_else(|err| {
        eprint!("invalid log filter: {err}");
        eprint!("using default log filter: info");
        EnvFilter::new("info")
    });
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr);
    if args.json {
        subscriber.json().init();
    } else {
        subscriber.init();
    }
}

pub async fn get_addr(host: &str, port: u16) -> Result<SocketAddr, ApiError> {
    let addrs = format!("{}:{}", host, port)
        .to_socket_addrs()
        .map_err(|e| ApiError::InternalServerError {
            reason: format!("Failed to resolve address: {}", e),
        })?
        .collect::<Vec<SocketAddr>>();

    let socket = match addrs.first() {
        Some(addr) => *addr,
        None => {
            return Err(ApiError::InternalServerError {
                reason: "No socket adresses found".into(),
            });
        }
    };

    Ok(socket)
}

pub async fn run_server(addr: SocketAddr, router: Router) {
    info!("listening on {addr}");

    axum_server::bind(addr)
        .serve(router.into_make_service())
        .await
        .expect("error when start server")
}

#[derive(Debug)]
pub enum MiddlewareError {
    MissingAuthHeader,
    InvalidAuthHeader,
    AuthenticationFailed(CoreError),
}

impl From<MiddlewareError> for StatusCode {
    fn from(value: MiddlewareError) -> Self {
        match value {
            MiddlewareError::MissingAuthHeader => StatusCode::UNAUTHORIZED,
            MiddlewareError::InvalidAuthHeader => StatusCode::UNAUTHORIZED,
            MiddlewareError::AuthenticationFailed(_) => StatusCode::UNAUTHORIZED,
        }
    }
}

impl IntoResponse for MiddlewareError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            MiddlewareError::MissingAuthHeader => (
                StatusCode::UNAUTHORIZED,
                "E_MISSING_AUTH_HEADER",
                "Authorization header is missing",
            ),
            MiddlewareError::InvalidAuthHeader => (
                StatusCode::UNAUTHORIZED,
                "E_INVALID_AUTH_HEADER",
                "Invalid authorization header",
            ),
            MiddlewareError::AuthenticationFailed(_) => (
                StatusCode::UNAUTHORIZED,
                "E_AUTHENTICATION_FAILED",
                "Authentication failed",
            ),
        };

        (status, Json(ApiErrorResponse::new(code, status, message))).into_response()
    }
}

pub async fn extract_token_from_bearer(auth_header: &HeaderValue) -> Result<Token, ApiError> {
    let auth_str = auth_header.to_str().map_err(|_| ApiError::TokenNotFound)?;

    if !auth_str.starts_with("Bearer ") {
        return Err(ApiError::TokenNotFound);
    }

    let token = auth_str
        .strip_prefix("Bearer ")
        .ok_or(ApiError::TokenNotFound)?;

    Ok(Token::new(token.to_string()))
}

pub async fn auth_middleware<T>(
    State(state): State<T>,
    req: Request,
    next: Next,
) -> Result<Response, MiddlewareError>
where
    T: AuthService + Clone + Send + Sync + 'static,
{
    auth_middleware_impl(State(state), req, |req| next.run(req)).await
}

pub(crate) async fn auth_middleware_impl<T, F, Fut>(
    State(state): State<T>,
    mut req: Request,
    next: F,
) -> Result<Response, MiddlewareError>
where
    T: AuthService + Clone + Send + Sync + 'static,
    F: FnOnce(Request) -> Fut,
    Fut: std::future::Future<Output = Response> + Send,
{
    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .ok_or(MiddlewareError::MissingAuthHeader)?;

    let token = extract_token_from_bearer(auth_header)
        .await
        .map_err(|_| MiddlewareError::InvalidAuthHeader)?;

    let identity = state.get_identity(token.as_str()).await.map_err(|e| {
        error!("Auth middleware: failed to identify user {:?}", e);
        MiddlewareError::AuthenticationFailed(e)
    })?;

    req.extensions_mut().insert(identity);

    Ok(next(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_auth::{Identity, User};
    use aether_core::CoreError;
    use axum::body::Body;
    use axum::http::HeaderValue;
    use axum::response::IntoResponse;
    use std::future::Future;

    #[tokio::test]
    async fn extract_token_from_bearer_success() {
        let header = HeaderValue::from_str("Bearer token-123").unwrap();
        let token = extract_token_from_bearer(&header).await.unwrap();
        assert_eq!(token.as_str(), "token-123");
    }

    #[tokio::test]
    async fn extract_token_from_bearer_rejects_missing_prefix() {
        let header = HeaderValue::from_str("Token abc").unwrap();
        let result = extract_token_from_bearer(&header).await;
        assert!(matches!(result, Err(ApiError::TokenNotFound)));
    }

    #[tokio::test]
    async fn extract_token_from_bearer_rejects_invalid_header() {
        let header = HeaderValue::from_bytes(b"\xFF").unwrap();
        let result = extract_token_from_bearer(&header).await;
        assert!(matches!(result, Err(ApiError::TokenNotFound)));
    }

    #[tokio::test]
    async fn get_addr_rejects_invalid_host() {
        let result = get_addr("invalid host", 1234).await;
        assert!(matches!(result, Err(ApiError::InternalServerError { .. })));
    }

    #[tokio::test]
    async fn get_addr_accepts_valid_host() {
        let result = get_addr("127.0.0.1", 0).await;
        assert!(result.is_ok());
    }

    #[test]
    fn middleware_error_status_codes() {
        assert_eq!(
            StatusCode::from(MiddlewareError::MissingAuthHeader),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            StatusCode::from(MiddlewareError::InvalidAuthHeader),
            StatusCode::UNAUTHORIZED
        );
    }

    #[test]
    fn middleware_error_into_response_is_unauthorized() {
        let response = MiddlewareError::MissingAuthHeader.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = MiddlewareError::InvalidAuthHeader.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
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

    #[tokio::test]
    async fn auth_middleware_impl_rejects_missing_header() {
        let state = FakeAuthService { fail: false };
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();

        let result = auth_middleware_impl(State(state), req, |_req| async move {
            StatusCode::OK.into_response()
        })
        .await;

        assert!(matches!(result, Err(MiddlewareError::MissingAuthHeader)));
    }

    #[tokio::test]
    async fn auth_middleware_impl_rejects_invalid_token() {
        let state = FakeAuthService { fail: false };
        let req = Request::builder()
            .uri("/")
            .header(AUTHORIZATION, "Token abc")
            .body(Body::empty())
            .unwrap();

        let result = auth_middleware_impl(State(state), req, |_req| async move {
            StatusCode::OK.into_response()
        })
        .await;

        assert!(matches!(result, Err(MiddlewareError::InvalidAuthHeader)));
    }

    #[tokio::test]
    async fn auth_middleware_impl_rejects_failed_identity() {
        let state = FakeAuthService { fail: true };
        let req = Request::builder()
            .uri("/")
            .header(AUTHORIZATION, "Bearer token")
            .body(Body::empty())
            .unwrap();

        let result = auth_middleware_impl(State(state), req, |_req| async move {
            StatusCode::OK.into_response()
        })
        .await;

        assert!(matches!(
            result,
            Err(MiddlewareError::AuthenticationFailed(_))
        ));
    }

    #[tokio::test]
    async fn auth_middleware_impl_inserts_identity_on_success() {
        let state = FakeAuthService { fail: false };
        let req = Request::builder()
            .uri("/")
            .header(AUTHORIZATION, "Bearer token")
            .body(Body::empty())
            .unwrap();

        let result = auth_middleware_impl(State(state), req, |req| async move {
            let identity = req.extensions().get::<Identity>().cloned();
            let response = if identity.is_some() {
                StatusCode::OK
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            response.into_response()
        })
        .await;

        assert!(matches!(result, Ok(response) if response.status() == StatusCode::OK));
    }
}
