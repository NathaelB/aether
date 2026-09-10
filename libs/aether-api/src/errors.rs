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

    /// The request was understood and cannot be satisfied in the current state
    /// of the installation -- no data plane with room, no provisioner to make
    /// one. Not the caller's mistake, so not a 400; not a server fault either,
    /// so not a 500.
    #[error("{reason}")]
    Conflict { reason: String },

    #[error("{reason}")]
    NotFound { reason: String },
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
            ApiError::Conflict { reason } => (
                StatusCode::CONFLICT,
                Json(ApiErrorResponse::new(
                    "E_CONFLICT",
                    StatusCode::CONFLICT,
                    reason,
                )),
            )
                .into_response(),
            ApiError::NotFound { reason } => (
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse::new(
                    "E_NOT_FOUND",
                    StatusCode::NOT_FOUND,
                    reason,
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

            // Placement failures are the caller's business, and each carries a
            // message written to be read. They used to fall through to the
            // catch-all below and come back as "an unexpected error occurred" --
            // so asking for a region nothing serves, or for a dedicated
            // deployment this installation cannot provision, looked exactly like
            // a bug in the control plane.
            CoreError::UnknownRegion { .. } => ApiError::BadRequest {
                reason: value.to_string(),
            },
            CoreError::NoDataPlaneAvailable { .. } | CoreError::ProvisioningUnavailable { .. } => {
                ApiError::Conflict {
                    reason: value.to_string(),
                }
            }

            // Same reasoning for the catalogue. Publishing a version twice and
            // moving a release backwards are both things the caller can see and
            // correct, and each error already says which release and why.
            CoreError::ReleaseAlreadyExists { .. } | CoreError::InvalidReleaseTransition { .. } => {
                ApiError::Conflict {
                    reason: value.to_string(),
                }
            }
            CoreError::ReleaseNotFound { .. } => ApiError::NotFound {
                reason: value.to_string(),
            },

            // Everything else stays deliberately opaque: a database error or an
            // internal invariant is not something a caller can act on, and its
            // message may name things the caller should not see.
            _ => ApiError::Unknown {
                reason: "an unexpected error occurred".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    /// The regression this replaced: every placement failure came back as
    /// "unknown error: an unexpected error occurred", so a region nothing
    /// serves was indistinguishable from a bug in the control plane.
    #[test]
    fn placement_failures_keep_their_message_and_get_a_status_that_means_something() {
        let response = ApiError::from(CoreError::UnknownRegion {
            region: "eu-west-9".to_string(),
        })
        .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response = ApiError::from(CoreError::NoDataPlaneAvailable {
            region: "local".to_string(),
            mode: "shared".to_string(),
        })
        .into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);

        let response = ApiError::from(CoreError::ProvisioningUnavailable {
            reason: "no provisioner".to_string(),
        })
        .into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn a_placement_failure_carries_the_reason_the_domain_wrote() {
        let error = ApiError::from(CoreError::UnknownRegion {
            region: "eu-west-9".to_string(),
        });

        assert!(
            error.to_string().contains("eu-west-9"),
            "the region must survive the conversion: {error}"
        );
    }

    /// The other half of the rule: an internal fault stays opaque. Its message
    /// may name a table, a query or a host, and none of that is the caller's.
    #[test]
    fn an_internal_error_does_not_leak_its_message() {
        let error = ApiError::from(CoreError::DatabaseError {
            message: "relation \"deployments\" does not exist".to_string(),
        });

        assert!(!error.to_string().contains("deployments"), "{error}");
    }

    #[test]
    fn api_error_into_response_status_codes() {
        let response = ApiError::BadRequest {
            reason: "bad".to_string(),
        }
        .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response = ApiError::TokenNotFound.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = ApiError::Forbidden {
            reason: "nope".to_string(),
        }
        .into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let response = ApiError::InternalServerError {
            reason: "boom".to_string(),
        }
        .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn core_error_maps_to_api_error() {
        let err = CoreError::FailedCreateOrganisation {
            organisation_name: "org".to_string(),
            reason: "bad".to_string(),
        };
        assert!(matches!(ApiError::from(err), ApiError::BadRequest { .. }));

        let err = CoreError::PermissionDenied {
            reason: "no".to_string(),
        };
        assert!(matches!(ApiError::from(err), ApiError::Forbidden { .. }));

        let err = CoreError::DatabaseError {
            message: "db".to_string(),
        };
        assert!(matches!(ApiError::from(err), ApiError::Unknown { .. }));
    }

    /// The catch-all below turns anything unmapped into "an unexpected error
    /// occurred". That already happened once to the placement errors, and it
    /// makes a caller's own mistake look like a bug in the control plane.
    #[test]
    fn a_catalogue_error_reaches_the_caller_intact() {
        let already = ApiError::from(CoreError::ReleaseAlreadyExists {
            release: "ferriskey 26.0.1".to_string(),
        });
        assert!(
            matches!(&already, ApiError::Conflict { reason } if reason.contains("26.0.1")),
            "{already:?}"
        );

        let backwards = ApiError::from(CoreError::InvalidReleaseTransition {
            release: "ferriskey 26.0.1".to_string(),
            from: "withdrawn".to_string(),
            to: "available".to_string(),
        });
        assert!(
            matches!(&backwards, ApiError::Conflict { reason } if reason.contains("withdrawn")),
            "{backwards:?}"
        );

        let absent = ApiError::from(CoreError::ReleaseNotFound {
            release: "ferriskey 99.0.0".to_string(),
        });
        assert!(
            matches!(&absent, ApiError::NotFound { reason } if reason.contains("99.0.0")),
            "{absent:?}"
        );
    }
}
