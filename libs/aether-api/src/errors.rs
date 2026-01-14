use aether_core::CoreError;
use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use thiserror::Error;
use utoipa::ToSchema;

#[derive(Debug, Error, ToSchema)]
pub enum ApiError {
    #[error("token not found")]
    TokenNotFound,

    #[error("bad request: {reason}")]
    BadRequest { reason: String },

    #[error("unknown error: {reason}")]
    Unknown { reason: String },

    #[error("internal server error: {reason}")]
    InternalServerError { reason: String },

    #[error("forbidden: {reason}")]
    Forbidden { reason: String },
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
            ApiError::BadRequest { reason } => (
                StatusCode::BAD_REQUEST,
                Json(ApiErrorResponse::new(
                    "E_BAD_REQUEST",
                    StatusCode::BAD_REQUEST,
                    format!("bad request: {reason}"),
                )),
            )
                .into_response(),
            ApiError::Forbidden { reason } => (
                StatusCode::FORBIDDEN,
                Json(ApiErrorResponse::new(
                    "E_FORBIDDEN",
                    StatusCode::FORBIDDEN,
                    format!("forbidden: {reason}"),
                )),
            )
                .into_response(),
        }
    }
}

impl From<CoreError> for ApiError {
    fn from(value: CoreError) -> Self {
        match value {
            CoreError::FailedCreateOrganisation {
                organisation_name: _,
                reason,
            } => ApiError::BadRequest { reason },
            CoreError::PermissionDenied { reason } => ApiError::Forbidden { reason },
            _ => ApiError::Unknown {
                reason: "an unexpected error occurred".to_string(),
            },
        }
    }
}
