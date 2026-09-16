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
            CoreError::ReleaseNotFound { .. }
            | CoreError::DeploymentNotFound { .. }
            | CoreError::BackupNotFound { .. }
            | CoreError::OrganisationNotFound { .. }
            | CoreError::MemberNotFound { .. } => ApiError::NotFound {
                reason: value.to_string(),
            },

            // Nothing the caller can change about the request. The archive is
            // real and they may restore it; it does not fit what it would be
            // restored onto, and the message says which half disagrees.
            CoreError::BackupNotRestorable { .. }
            | CoreError::BackupFromALaterRelease { .. }
            | CoreError::BackupLockedToPostgresMajor { .. } => ApiError::Conflict {
                reason: value.to_string(),
            },

            // Not a 403: no role opens this, only a different plan does, and
            // the message names which one.
            CoreError::OfferNotOpenToPlan { .. } => ApiError::Conflict {
                reason: value.to_string(),
            },

            // A 403, and the message names the right. A refusal saying only
            // "forbidden" sends an operator looking for a role inside an
            // organisation, which is a different system answering a different
            // question.
            CoreError::MissingPlatformRight { .. } => ApiError::Forbidden {
                reason: value.to_string(),
            },

            // Not a 403 either: the caller is allowed to grant, and this
            // particular grant is refused. A 403 would have them ask for a
            // right they already hold.
            CoreError::CannotGrantWhatYouDoNotHold { .. }
            | CoreError::LastOperatorCannotBeRemoved
            // Not a 400 either: the plan is a real one and the caller may
            // choose it. What is in the way is the tenant, and the message
            // names it.
            | CoreError::PlanBelowWhatIsInUse { .. } => ApiError::Conflict {
                reason: value.to_string(),
            },

            CoreError::UnknownPlatformRight { .. } => ApiError::BadRequest {
                reason: value.to_string(),
            },

            // State the caller can see and act on: an archive is on its way,
            // and the message says when it was asked for.
            CoreError::BackupAlreadyUnderway { .. } => ApiError::Conflict {
                reason: value.to_string(),
            },

            // Not a permission the caller could go and ask for: nobody holds
            // one that removes an owner. A 403 would send them looking for a
            // role that does not exist.
            CoreError::OwnerCannotBeRemoved { .. } => ApiError::Conflict {
                reason: value.to_string(),
            },

            // The caller sent a role id; which one was wrong is the only
            // thing they need, and it is in the message.
            CoreError::RoleNotInOrganisation { .. } | CoreError::NotAnEmail { .. } => {
                ApiError::BadRequest {
                    reason: value.to_string(),
                }
            }

            // Says nothing about which of "never existed", "was revoked" or
            // "belongs to another organisation" it is. Telling them apart
            // lets somebody holding a wrong link learn which links are real.
            CoreError::InvitationNotFound => ApiError::NotFound {
                reason: value.to_string(),
            },

            // Told apart from the one above on purpose: somebody who really
            // was invited needs to know to ask for another link rather than
            // go hunting for a typo they did not make.
            CoreError::InvitationExpired { .. }
            | CoreError::InvitationAlreadyAccepted
            | CoreError::InvitationRevoked
            | CoreError::AlreadyAMember { .. } => ApiError::Conflict {
                reason: value.to_string(),
            },

            // Forbidden rather than not-found: the link is real, and saying
            // so costs nothing -- whoever is holding it already has it. What
            // they do not get is somebody else's place.
            CoreError::InvitationAddressedToSomebodyElse => ApiError::Forbidden {
                reason: value.to_string(),
            },

            // An upgrade refused because the deployment is busy, or because the
            // target must not be installed, is state the caller can see and act
            // on. A version that is not ahead is the caller's own mistake.
            CoreError::DeploymentNotUpgradable { .. }
            | CoreError::DeploymentBusy { .. }
            | CoreError::ReleaseNotInstallable { .. } => ApiError::Conflict {
                reason: value.to_string(),
            },
            // The cap is the server's answer to a request that was
            // understood, so it says so plainly rather than falling through
            // to the catch-all.
            CoreError::InvalidLogWindow { .. } => ApiError::BadRequest {
                reason: value.to_string(),
            },
            CoreError::Version(_) => ApiError::BadRequest {
                reason: value.to_string(),
            },

            // Everything else stays deliberately opaque to the caller: a
            // database error or an internal invariant is not something they
            // can act on, and its message may name things they should not see.
            //
            // Logged on the way past, because opaque to the caller is not the
            // same as lost. Without this line the only evidence of a bug is a
            // 400 saying nothing, and finding out which error it was means
            // adding this line and deploying again -- which is exactly how
            // this one came to be written.
            other => {
                tracing::error!(error = %other, "a request failed with no answer for the caller");

                ApiError::Unknown {
                    reason: "an unexpected error occurred".to_string(),
                }
            }
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
        let capped = ApiError::from(CoreError::InvalidLogWindow {
            requested: 1440,
            max: 60,
        });
        assert!(matches!(capped, ApiError::BadRequest { .. }));
        assert!(capped.to_string().contains("60"), "{capped}");

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

    /// Same reasoning as the catalogue and placement errors before it: these
    /// are things the caller can correct, and the catch-all would turn every
    /// one of them into "an unexpected error occurred".
    #[test]
    fn an_upgrade_refusal_reaches_the_caller_intact() {
        let busy = ApiError::from(CoreError::DeploymentNotUpgradable {
            deployment: uuid::Uuid::nil(),
            status: "deleting".to_string(),
        });
        assert!(
            matches!(&busy, ApiError::Conflict { reason } if reason.contains("deleting")),
            "{busy:?}"
        );

        let withdrawn = ApiError::from(CoreError::ReleaseNotInstallable {
            release: "ferriskey 25.0.0".to_string(),
            status: "withdrawn".to_string(),
        });
        assert!(
            matches!(&withdrawn, ApiError::Conflict { reason } if reason.contains("withdrawn")),
            "{withdrawn:?}"
        );

        let backwards = ApiError::from(CoreError::Version(
            aether_core::version::VersionError::NotAhead {
                from: "26.1.0".to_string(),
                to: "26.0.1".to_string(),
            },
        ));
        assert!(
            matches!(&backwards, ApiError::BadRequest { reason } if reason.contains("downgrade")),
            "{backwards:?}"
        );

        let absent = ApiError::from(CoreError::DeploymentNotFound {
            id: uuid::Uuid::nil(),
        });
        assert!(matches!(absent, ApiError::NotFound { .. }), "{absent:?}");

        let busy = ApiError::from(CoreError::DeploymentBusy {
            deployment: uuid::Uuid::nil(),
            status: "upgrading".to_string(),
            operation: "deleted".to_string(),
        });
        assert!(
            matches!(&busy, ApiError::Conflict { reason } if reason.contains("upgrading")),
            "{busy:?}"
        );
    }

    /// The three deployment endpoints used to discard the error and answer
    /// "deployment not found" with a 400 whatever had actually happened. The
    /// domain now says which it was, and this is what carries it through.
    /// Removing an owner is refused for a reason nobody can be granted past,
    /// so it is a conflict rather than a permission problem. A 403 would send
    /// somebody looking for a role that does not exist.
    #[test]
    fn removing_an_owner_is_a_conflict_and_says_so() {
        let refused = ApiError::from(CoreError::OwnerCannotBeRemoved {
            organisation: uuid::Uuid::nil(),
        });

        let ApiError::Conflict { reason } = &refused else {
            panic!("expected a conflict, got {refused:?}");
        };
        assert!(reason.contains("owner"), "{reason}");
    }

    /// The caller sent a list of role ids; which one was wrong is the only
    /// thing they need back.
    #[test]
    fn a_role_from_elsewhere_is_a_bad_request_naming_it() {
        let role = uuid::Uuid::from_u128(7);
        let refused = ApiError::from(CoreError::RoleNotInOrganisation {
            organisation: uuid::Uuid::nil(),
            role,
        });

        let ApiError::BadRequest { reason } = &refused else {
            panic!("expected a bad request, got {refused:?}");
        };
        assert!(reason.contains(&role.to_string()), "{reason}");
    }

    #[test]
    fn somebody_who_is_not_a_member_is_not_found() {
        let absent = ApiError::from(CoreError::MemberNotFound {
            organisation: uuid::Uuid::nil(),
            user: uuid::Uuid::nil(),
        });

        assert!(matches!(absent, ApiError::NotFound { .. }), "{absent:?}");
    }

    /// A restore refused because the archive does not fit is an answer, not a
    /// fault: as a 500 it reads as the platform breaking during an outage,
    /// which is exactly when a restore is asked for.
    #[test]
    fn an_archive_that_does_not_fit_is_a_conflict_rather_than_a_fault() {
        for refused in [
            CoreError::BackupFromALaterRelease {
                backup: uuid::Uuid::nil(),
                taken_on: "keycloak 26.1.0".to_string(),
                target: "keycloak 26.0.0".to_string(),
            },
            CoreError::BackupNotRestorable {
                backup: uuid::Uuid::nil(),
                reason: "it was taken of another deployment".to_string(),
            },
        ] {
            let refused = ApiError::from(refused);

            assert_eq!(
                refused.into_response().status(),
                StatusCode::CONFLICT,
                "an archive that does not fit was not a conflict"
            );
        }
    }

    /// Asking twice is state the caller can act on -- the first archive is on
    /// its way -- rather than a fault. As a 400 with no message it reads as
    /// the request having been malformed.
    #[test]
    fn a_second_ask_while_one_is_coming_is_a_conflict_that_says_when() {
        let refused = ApiError::from(CoreError::BackupAlreadyUnderway {
            since: "2026-09-01T12:09:00Z".to_string(),
        });

        let ApiError::Conflict { reason } = refused else {
            panic!("asking twice was not a conflict");
        };
        assert!(reason.contains("12:09"), "got {reason}");
    }

    /// Reached the caller as an opaque 400 while the endpoint documented a
    /// 404, so following a stale link looked like a malformed request.
    #[test]
    fn an_organisation_nobody_has_is_not_found() {
        let absent = ApiError::from(CoreError::OrganisationNotFound {
            id: uuid::Uuid::nil(),
        });

        assert_eq!(absent.into_response().status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn an_archive_nobody_has_is_not_found() {
        let absent = ApiError::from(CoreError::BackupNotFound {
            id: uuid::Uuid::nil(),
        });

        assert_eq!(absent.into_response().status(), StatusCode::NOT_FOUND);
    }

    /// Every one of these reached the caller as a 500 "unknown error" while
    /// the endpoint documented a 403 and a 409, which is how a refusal that
    /// works reads as a platform that is broken.
    #[test]
    fn a_platform_refusal_says_which_kind_of_refusal_it_is() {
        for (refused, expected) in [
            (
                CoreError::PlanBelowWhatIsInUse {
                    plan: "starter".to_string(),
                    what: "deployments".to_string(),
                    allowed: 5,
                    in_use: 8,
                },
                StatusCode::CONFLICT,
            ),
            (
                CoreError::MissingPlatformRight {
                    right: "manage_operators".to_string(),
                },
                StatusCode::FORBIDDEN,
            ),
            (
                CoreError::CannotGrantWhatYouDoNotHold {
                    rights: "act_on_tenant".to_string(),
                },
                StatusCode::CONFLICT,
            ),
            (CoreError::LastOperatorCannotBeRemoved, StatusCode::CONFLICT),
            (
                CoreError::UnknownPlatformRight {
                    value: "root".to_string(),
                },
                StatusCode::BAD_REQUEST,
            ),
        ] {
            let said = ApiError::from(refused);
            let carries = said.to_string();

            assert_eq!(said.into_response().status(), expected, "{carries}");
        }
    }

    /// The refusal names the right. Without the name, an operator reads
    /// "forbidden" and goes looking through their organisation's roles.
    #[test]
    fn a_missing_right_is_named_in_the_refusal() {
        let refused = ApiError::from(CoreError::MissingPlatformRight {
            right: "act_on_tenant".to_string(),
        });

        let ApiError::Forbidden { reason } = refused else {
            panic!("a missing platform right was not a 403");
        };
        assert!(reason.contains("act_on_tenant"), "got {reason}");
    }

    #[test]
    fn a_missing_deployment_is_not_found_rather_than_a_bad_request() {
        let absent = ApiError::from(CoreError::DeploymentNotFound {
            id: uuid::Uuid::nil(),
        });

        assert!(matches!(absent, ApiError::NotFound { .. }), "{absent:?}");
        assert_eq!(
            absent.into_response().status(),
            StatusCode::NOT_FOUND,
            "the caller sees 404"
        );
    }
}
