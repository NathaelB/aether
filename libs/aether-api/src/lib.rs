use std::net::{SocketAddr, ToSocketAddrs};

use aether_auth::Token;
use aether_core::{CoreError, auth::ports::AuthService};
use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::Response,
};

use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use crate::{args::LogArgs, errors::ApiError};

pub mod args;
pub mod auth;
pub mod errors;
pub mod response;
pub mod router;
pub mod state;

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
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode>
where
    T: AuthService + Clone + Send + Sync + 'static,
{
    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .ok_or(MiddlewareError::MissingAuthHeader)?;

    let token = extract_token_from_bearer(auth_header)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let identity = state.get_identity(token.as_str()).await.map_err(|e| {
        error!("Auth middleware: failed to identity user {:?}", e);
        MiddlewareError::AuthenticationFailed(e)
    })?;

    req.extensions_mut().insert(identity);

    Ok(next.run(req).await)
}
