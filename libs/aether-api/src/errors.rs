use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("token not found")]
    TokenNotFound,

    #[error("unknown error: {reason}")]
    Unknown { reason: String },

    #[error("internal server error: {reason}")]
    InternalServerError { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApiErrorResponse {
    pub code: String,
    pub status: u16,
    pub message: String,
}

impl ApiErrorResponse {
    pub fn new(code: impl Into<String>, status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            status: status.as_u16(),
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        match self {
            ApiError::InternalServerError { reason } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new(
                    "E_INTERNAL_SERVER_ERROR",
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("internal server error: {reason}"),
                )),
            )
                .into_response(),
            ApiError::Unknown { reason } => (
                StatusCode::BAD_REQUEST,
                Json(ApiErrorResponse::new(
                    "E_UNKNOWN",
                    StatusCode::BAD_REQUEST,
                    format!("unknown error: {reason}"),
                )),
            )
                .into_response(),
            ApiError::TokenNotFound => (
                StatusCode::UNAUTHORIZED,
                Json(ApiErrorResponse::new(
                    "E_TOKEN_NOT_FOUND",
                    StatusCode::UNAUTHORIZED,
                    "token not found",
                )),
            )
                .into_response(),
        }
    }
}
