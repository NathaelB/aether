use axum::{
    body::to_bytes,
    http::{HeaderValue, StatusCode},
    response::IntoResponse,
};

use aether_api::{
    MiddlewareError,
    extract_token_from_bearer,
    get_addr,
    errors::ApiError,
    handlers::default_limit,
};

#[tokio::test]
async fn extract_token_from_bearer_accepts_valid_header() {
    let header = HeaderValue::from_static("Bearer test-token");
    let token = extract_token_from_bearer(&header).await.expect("token");

    assert_eq!(token.as_str(), "test-token");
}

#[tokio::test]
async fn extract_token_from_bearer_rejects_invalid_header() {
    let header = HeaderValue::from_static("Token test-token");
    let err = extract_token_from_bearer(&header).await.expect_err("error");

    match err {
        ApiError::TokenNotFound => {}
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn api_error_into_response_contains_code() {
    let response = ApiError::TokenNotFound.into_response();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    let body_str = std::str::from_utf8(&body).expect("utf8");

    assert!(body_str.contains("\"code\":\"E_TOKEN_NOT_FOUND\""));
}

#[tokio::test]
async fn middleware_error_into_response_contains_code() {
    let response = MiddlewareError::InvalidAuthHeader.into_response();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    let body_str = std::str::from_utf8(&body).expect("utf8");

    assert!(body_str.contains("\"code\":\"E_INVALID_AUTH_HEADER\""));
}

#[tokio::test]
async fn get_addr_resolves_localhost() {
    let addr = get_addr("127.0.0.1", 8080).await.expect("addr");

    assert_eq!(addr.ip().to_string(), "127.0.0.1");
    assert_eq!(addr.port(), 8080);
}

#[test]
fn default_limit_is_stable() {
    assert_eq!(default_limit(), 10);
}
