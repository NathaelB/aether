use aether_auth::Identity;
use aether_core::{
    deployments::DeploymentId,
    iam_settings::{
        IamSettings,
        branding::{Branding, BrandingInput},
        ports::{IamSettingsService, IamSettingsState},
    },
    organisation::OrganisationId,
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments/{deployment_id}/iam-settings")]
pub struct IamSettingsRoute {
    pub organisation_id: Uuid,
    pub deployment_id: Uuid,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SetIamSettingsRequest {
    pub branding: Option<BrandingInput>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq)]
pub struct IamSettingsResponse {
    pub data: IamSettingsState,
}

fn settings_from(request: SetIamSettingsRequest) -> Result<IamSettings, ApiError> {
    let branding = request
        .branding
        .map(Branding::try_from)
        .transpose()
        .map_err(|e| ApiError::BadRequest {
            reason: e.to_string(),
        })?;

    Ok(IamSettings { branding })
}

async fn read_settings<S: IamSettingsService>(
    service: &S,
    identity: Identity,
    organisation_id: Uuid,
    deployment_id: Uuid,
) -> Result<Response<IamSettingsResponse>, ApiError> {
    let state = service
        .iam_settings(
            identity,
            OrganisationId(organisation_id),
            DeploymentId(deployment_id),
        )
        .await?;

    Ok(Response::OK(IamSettingsResponse { data: state }))
}

async fn replace_settings<S: IamSettingsService>(
    service: &S,
    identity: Identity,
    organisation_id: Uuid,
    deployment_id: Uuid,
    request: SetIamSettingsRequest,
) -> Result<Response<IamSettingsResponse>, ApiError> {
    let settings = settings_from(request)?;

    let state = service
        .set_iam_settings(
            identity,
            OrganisationId(organisation_id),
            DeploymentId(deployment_id),
            settings,
        )
        .await?;

    Ok(Response::Accepted(IamSettingsResponse { data: state }))
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/deployments/{deployment_id}/iam-settings",
    summary = "read an instance's identity settings and where the latest request stands",
    tag = "deployments",
    description = "`branding` is the desired state. `request` is the latest request to apply it, or null when none was made: its status is the control plane's own, and `Published` means it reached the data plane, not that the instance accepted it.",
    params(IamSettingsRoute),
    responses(
        (status = 200, description = "The desired settings and the latest request", body = IamSettingsResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not view this deployment", body = ApiError),
        (status = 404, description = "No such deployment", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_iam_settings_handler(
    IamSettingsRoute {
        organisation_id,
        deployment_id,
    }: IamSettingsRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<IamSettingsResponse>, ApiError> {
    read_settings(&state.service, identity, organisation_id, deployment_id).await
}

#[utoipa::path(
    put,
    path = "/{organisation_id}/deployments/{deployment_id}/iam-settings",
    summary = "replace an instance's identity settings",
    tag = "deployments",
    description = "Replaces the whole desired state and asks the data plane to apply it. Answers 202 with the request just made: applying it is asynchronous. Colors are `#rrggbb`, radius an integer from 0 to 24.",
    params(IamSettingsRoute),
    request_body = SetIamSettingsRequest,
    responses(
        (status = 202, description = "Stored, and a request to apply it was made", body = IamSettingsResponse),
        (status = 400, description = "A color or the radius is not valid", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not change this deployment, or the plan does not open branding", body = ApiError),
        (status = 404, description = "No such deployment", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn set_iam_settings_handler(
    IamSettingsRoute {
        organisation_id,
        deployment_id,
    }: IamSettingsRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<SetIamSettingsRequest>,
) -> Result<Response<IamSettingsResponse>, ApiError> {
    replace_settings(
        &state.service,
        identity,
        organisation_id,
        deployment_id,
        request,
    )
    .await
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use aether_core::{CoreError, action::ActionStatus, iam_settings::ports::IamSettingsRequest};
    use axum::{http::StatusCode, response::IntoResponse};
    use serde_json::json;

    use super::*;
    use crate::test_helpers::user_identity;

    struct StubService {
        refusal: Option<&'static str>,
        stored: Mutex<Vec<IamSettings>>,
    }

    impl StubService {
        fn accepting() -> Self {
            Self {
                refusal: None,
                stored: Mutex::new(Vec::new()),
            }
        }

        fn refusing(reason: &'static str) -> Self {
            Self {
                refusal: Some(reason),
                stored: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> usize {
            self.stored.lock().expect("not poisoned").len()
        }

        fn state(branding: Option<Branding>) -> IamSettingsState {
            IamSettingsState {
                branding,
                request: Some(IamSettingsRequest {
                    action_id: aether_core::action::ActionId(Uuid::nil()),
                    status: ActionStatus::Pending,
                    created_at: chrono::DateTime::UNIX_EPOCH,
                }),
            }
        }
    }

    impl IamSettingsService for StubService {
        async fn iam_settings(
            &self,
            _identity: Identity,
            _organisation_id: OrganisationId,
            _deployment_id: DeploymentId,
        ) -> Result<IamSettingsState, CoreError> {
            match self.refusal {
                Some(reason) => Err(CoreError::PermissionDenied {
                    reason: reason.to_string(),
                }),
                None => Ok(Self::state(None)),
            }
        }

        async fn set_iam_settings(
            &self,
            _identity: Identity,
            _organisation_id: OrganisationId,
            _deployment_id: DeploymentId,
            settings: IamSettings,
        ) -> Result<IamSettingsState, CoreError> {
            self.stored
                .lock()
                .expect("not poisoned")
                .push(settings.clone());
            match self.refusal {
                Some(reason) => Err(CoreError::PermissionDenied {
                    reason: reason.to_string(),
                }),
                None => Ok(Self::state(settings.branding)),
            }
        }
    }

    fn request(body: serde_json::Value) -> SetIamSettingsRequest {
        serde_json::from_value(body).expect("a body")
    }

    async fn put(service: &StubService, body: serde_json::Value) -> StatusCode {
        match replace_settings(
            service,
            user_identity("user"),
            Uuid::nil(),
            Uuid::nil(),
            request(body),
        )
        .await
        {
            Ok(response) => response.into_response().status(),
            Err(error) => error.into_response().status(),
        }
    }

    #[tokio::test]
    async fn a_valid_branding_is_accepted_with_a_202() {
        let service = StubService::accepting();

        let status = put(
            &service,
            json!({ "branding": { "colors": { "primary": "#112233" }, "radius": 6 } }),
        )
        .await;

        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(service.calls(), 1);
    }

    #[tokio::test]
    async fn null_branding_is_accepted_and_reaches_the_service_as_none() {
        let service = StubService::accepting();

        let status = put(&service, json!({ "branding": null })).await;

        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(
            service.stored.lock().expect("not poisoned").as_slice(),
            [IamSettings { branding: None }]
        );
    }

    #[tokio::test]
    async fn an_invalid_color_is_a_400_and_reaches_nothing() {
        let service = StubService::accepting();

        let status = put(
            &service,
            json!({ "branding": { "colors": { "links": "blue" } } }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(service.calls(), 0);
    }

    #[tokio::test]
    async fn an_invalid_radius_is_a_400_and_reaches_nothing() {
        let service = StubService::accepting();

        assert_eq!(
            put(&service, json!({ "branding": { "radius": 25 } })).await,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            put(&service, json!({ "branding": { "radius": -1 } })).await,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(service.calls(), 0);
    }

    #[test]
    fn the_refusal_names_the_color() {
        let error = settings_from(request(json!({
            "branding": { "colors": { "page_background": "nope" } }
        })))
        .expect_err("refused");

        let ApiError::BadRequest { reason } = error else {
            panic!("expected a bad request");
        };
        assert!(reason.contains("page_background"), "{reason}");
    }

    #[test]
    fn unknown_fields_are_not_read() {
        assert!(serde_json::from_value::<SetIamSettingsRequest>(json!({ "theme": {} })).is_err());
        assert!(
            serde_json::from_value::<SetIamSettingsRequest>(json!({
                "branding": { "fonts": {} }
            }))
            .is_err()
        );
    }

    #[tokio::test]
    async fn a_refusal_from_the_service_is_a_403() {
        let service = StubService::refusing("the free plan does not open branding");

        assert_eq!(
            put(&service, json!({ "branding": null })).await,
            StatusCode::FORBIDDEN
        );

        let read = read_settings(&service, user_identity("user"), Uuid::nil(), Uuid::nil()).await;
        assert_eq!(
            read.expect_err("refused").into_response().status(),
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn reading_answers_the_desired_state_and_the_request() {
        let service = StubService::accepting();

        let response = read_settings(&service, user_identity("user"), Uuid::nil(), Uuid::nil())
            .await
            .expect("read");

        let Response::OK(body) = response else {
            panic!("expected a 200");
        };
        let wire = serde_json::to_value(&body).unwrap();
        assert!(wire["data"]["branding"].is_null());
        assert_eq!(wire["data"]["request"]["status"], "Pending");
        assert!(wire["data"]["request"]["created_at"].is_string());
        assert!(wire["data"]["request"]["action_id"].is_string());
    }
}
