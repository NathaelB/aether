use aether_auth::Identity;
use aether_core::{
    organisation::{
        Organisation,
        commands::CreateOrganisationCommand,
        ports::OrganisationService,
        value_objects::{OrganisationName, OrganisationSlug, Plan},
    },
    user::UserId,
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use tracing::info;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Deserialize, ToSchema)]
pub struct CreateOrganisationRequest {
    pub name: String,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct CreateOrganisationResponse {
    data: Organisation,
}

#[derive(TypedPath)]
#[typed_path("/organisations")]
pub struct CreateOrganisationRoute;

#[utoipa::path(
    post,
    path = "",
    summary = "create organisation",
    tag = "organisation",
    description = "Create a new organisation. The authenticated user will become the owner.",
    request_body = CreateOrganisationRequest,
    responses(
        (status = 201, description = "Organisation created successfully", body = CreateOrganisationResponse),
        (status = 400, description = "Invalid request data", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_organisation_handler(
    _: CreateOrganisationRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<CreateOrganisationRequest>,
) -> Result<Response<CreateOrganisationResponse>, ApiError> {
    let name = OrganisationName::new(request.name).map_err(|e| ApiError::BadRequest {
        reason: e.to_string(),
    })?;

    let slug = OrganisationSlug::from_name(&name).map_err(|e| ApiError::BadRequest {
        reason: e.to_string(),
    })?;

    let plan = Plan::Free;

    let user_id = Uuid::parse_str(identity.id()).map_err(|e| ApiError::InternalServerError {
        reason: e.to_string(),
    })?;
    let owner_id = UserId(user_id);

    let command = CreateOrganisationCommand::new(name, owner_id, plan);
    let command = command.with_slug(slug);
    info!("try to create organisation: {:?}", command);

    let org = state.service.create_organisation(command).await?;

    info!("organisation created: {:?}", org);

    Ok(Response::Created(CreateOrganisationResponse { data: org }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{app_state, user_identity};

    #[tokio::test]
    async fn create_organisation_rejects_invalid_name() {
        let state = app_state();
        let identity = user_identity("user-123");
        let request = CreateOrganisationRequest {
            name: "a".to_string(),
        };

        let result = create_organisation_handler(
            CreateOrganisationRoute,
            State(state),
            Extension(identity),
            Json(request),
        )
        .await;

        assert!(matches!(result, Err(ApiError::BadRequest { .. })));
    }

    #[tokio::test]
    async fn create_organisation_rejects_invalid_identity() {
        let state = app_state();
        let identity = user_identity("not-a-uuid");
        let request = CreateOrganisationRequest {
            name: "Acme Corp".to_string(),
        };

        let result = create_organisation_handler(
            CreateOrganisationRoute,
            State(state),
            Extension(identity),
            Json(request),
        )
        .await;

        assert!(matches!(result, Err(ApiError::InternalServerError { .. })));
    }
}
