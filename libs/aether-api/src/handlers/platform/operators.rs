use aether_auth::Identity;
use aether_core::platform::{
    PlatformOperator, PlatformRight, PlatformRights, ports::PlatformService,
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath)]
#[typed_path("/platform/operators")]
pub struct OperatorsRoute;

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/platform/operators/{subject}")]
pub struct OperatorRoute {
    /// The subject the identity provider issues for this operator.
    pub subject: String,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct OperatorsResponse {
    pub data: Vec<PlatformOperator>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct OperatorResponse {
    pub data: PlatformOperator,
}

/// What somebody is to hold from now on.
///
/// The whole set, not a change to it. A request that added or removed one
/// right at a time would need two calls to swap a right for another, and an
/// operator would hold both or neither in between.
#[derive(Deserialize, ToSchema)]
pub struct GrantOperatorRequest {
    /// `view_estate`, `operate_fleet`, `act_on_tenant`, `manage_operators`.
    pub rights: Vec<String>,
}

#[utoipa::path(
    get,
    path = "/operators",
    summary = "list who operates this installation",
    tag = "platform",
    description = "Reading who operates the installation is part of seeing it: somebody who \
                   may look at every tenant learning who else may is not the leak.",
    responses(
        (status = 200, description = "Everybody granted a platform right", body = OperatorsResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "This needs the view_estate right", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_operators_handler(
    _: OperatorsRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<OperatorsResponse>, ApiError> {
    let operators = state.service.list_operators(identity).await?;

    Ok(Response::OK(OperatorsResponse { data: operators }))
}

#[utoipa::path(
    put,
    path = "/operators/{subject}",
    summary = "grant platform rights",
    tag = "platform",
    description = "Replaces whatever the subject held. Nobody grants a right they were not \
                   given themselves, and the last identity able to manage operators cannot \
                   be narrowed or removed.",
    params(OperatorRoute),
    request_body = GrantOperatorRequest,
    responses(
        (status = 200, description = "What the subject now holds", body = OperatorResponse),
        (status = 400, description = "The request names a right nobody grants", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "This needs the manage_operators right", body = ApiError),
        (status = 409, description = "The grant would leave nobody able to manage operators, or hands out a right the caller does not hold", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn grant_operator_handler(
    OperatorRoute { subject }: OperatorRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<GrantOperatorRequest>,
) -> Result<Response<OperatorResponse>, ApiError> {
    let operator = state
        .service
        .grant_operator(identity, subject, request.into_rights()?)
        .await?;

    Ok(Response::OK(OperatorResponse { data: operator }))
}

#[utoipa::path(
    delete,
    path = "/operators/{subject}",
    summary = "revoke every platform right",
    tag = "platform",
    description = "Takes effect on the subject's next request, not on their next token.",
    params(OperatorRoute),
    responses(
        (status = 204, description = "They operate nothing here now"),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "This needs the manage_operators right", body = ApiError),
        (status = 409, description = "This is the last identity able to manage operators", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn revoke_operator_handler(
    OperatorRoute { subject }: OperatorRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<()>, ApiError> {
    state.service.revoke_operator(identity, subject).await?;

    Ok(Response::NoContent)
}

impl GrantOperatorRequest {
    /// Reads the rights, naming the one that could not be read.
    ///
    /// Refused rather than skipped: a request naming three rights and one
    /// typo would otherwise grant two and report success, and nobody reads a
    /// 200 to find out what it left out.
    fn into_rights(self) -> Result<PlatformRights, ApiError> {
        let rights = self
            .rights
            .iter()
            .map(|right| {
                right
                    .parse::<PlatformRight>()
                    .map_err(|_| ApiError::BadRequest {
                        reason: format!("'{right}' is not a platform right"),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(PlatformRights::of(rights))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_right_nobody_grants_is_refused_rather_than_skipped() {
        let refused = GrantOperatorRequest {
            rights: vec!["view_estate".to_string(), "root".to_string()],
        }
        .into_rights()
        .expect_err("a right nobody grants was accepted");

        let ApiError::BadRequest { reason } = refused else {
            panic!("the refusal was not a bad request");
        };
        assert!(reason.contains("root"), "got {reason}");
    }

    #[test]
    fn the_rights_asked_for_are_the_rights_read() {
        let read = GrantOperatorRequest {
            rights: vec!["view_estate".to_string(), "act_on_tenant".to_string()],
        }
        .into_rights()
        .expect("two valid rights");

        assert!(read.holds(PlatformRight::ViewEstate));
        assert!(read.holds(PlatformRight::ActOnTenant));
        assert!(!read.holds(PlatformRight::ManageOperators));
    }

    /// An empty grant reaches the domain, which reads it as a revocation. The
    /// API does not second-guess that: two places deciding what an empty set
    /// means is how they come to disagree.
    #[test]
    fn an_empty_grant_is_carried_rather_than_refused_here() {
        assert!(
            GrantOperatorRequest { rights: vec![] }
                .into_rights()
                .expect("an empty grant")
                .is_empty()
        );
    }
}
