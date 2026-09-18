use aether_auth::Identity;
use aether_core::{
    dataplane::value_objects::Region,
    deployments::{
        Deployment, DeploymentKind, DeploymentName, commands::CreateDeploymentCommand,
        environment::Environment, ports::DeploymentService,
    },
    offers::Offer,
    user::UserId,
    version::Version,
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Deserialize, ToSchema)]
pub struct CreateDeploymentRequest {
    pub name: String,
    pub kind: String,
    pub version: String,

    /// `production`, `staging` or `development`.
    pub environment: String,

    /// Refused, and present only so that it is.
    ///
    /// Where a deployment runs is the installation's to decide: a region is
    /// the fleet seen from outside, and choosing one is choosing which
    /// cluster serves you. Dropping the field would let a caller that still
    /// sends one believe it was honoured, and a request that asked for one
    /// country and was quietly given another is the worst way to find out.
    ///
    /// It comes back the day regions are a product surface rather than a view
    /// of the fleet, and it comes back as a promise about where data lives.
    #[serde(default)]
    pub region: Option<String>,

    /// What they are buying: `sandbox`, `standard`, `scale` or `private`.
    ///
    /// The size and whether the deployment gets a cluster of its own are read
    /// from it. There is no field for either, deliberately: a request that
    /// could send both could contradict the offer it named, and nothing would
    /// be able to say which half was meant.
    pub offer: String,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct CreateDeploymentResponse {
    data: Deployment,
}

struct ParsedCreateDeploymentRequest {
    name: String,
    kind: DeploymentKind,
    version: Version,
    environment: Environment,
    region: Region,
    offer: Offer,
}

/// What a request naming a region is told.
const REGION_IS_NOT_YOURS: &str = "where a deployment runs is decided by the platform;                                    remove `region` from the request";

impl ParsedCreateDeploymentRequest {
    fn parse(request: CreateDeploymentRequest, default_region: &str) -> Result<Self, ApiError> {
        let refused = |reason: String| ApiError::BadRequest { reason };

        let kind =
            DeploymentKind::try_from(request.kind.as_str()).map_err(|e| refused(e.to_string()))?;

        let version = Version::parse(&request.version).map_err(|e| refused(e.to_string()))?;

        let environment = request
            .environment
            .parse::<Environment>()
            .map_err(|e| refused(e.to_string()))?;

        let offer = request
            .offer
            .parse::<Offer>()
            .map_err(|e| refused(e.to_string()))?;

        // Refused rather than ignored. A caller that named a region and was
        // silently given another would find out from where their data is.
        if request.region.is_some() {
            return Err(refused(REGION_IS_NOT_YOURS.to_string()));
        }

        Ok(Self {
            name: request.name,
            kind,
            version,
            environment,
            region: Region::new(default_region.to_string()),
            offer,
        })
    }
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments")]
pub struct CreateDeploymentRoute {
    pub organisation_id: Uuid,
}

#[utoipa::path(
    post,
    path = "/{organisation_id}/deployments",
    summary = "create deployment",
    tag = "deployments",
    description = "Create a deployment within the specified organisation. Where it runs is \
                   the installation's decision, not the caller's: a request that names a region \
                   is refused rather than having it quietly substituted.",
    request_body = CreateDeploymentRequest,
    params(CreateDeploymentRoute),
    responses(
        (status = 201, description = "Deployment created successfully", body = CreateDeploymentResponse),
        (status = 400, description = "Invalid request data", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_deployment_handler(
    CreateDeploymentRoute { organisation_id }: CreateDeploymentRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<CreateDeploymentRequest>,
) -> Result<Response<CreateDeploymentResponse>, ApiError> {
    let organisation_id = organisation_id.into();
    let created_by =
        identity
            .id()
            .parse::<UserId>()
            .map_err(|e| ApiError::InternalServerError {
                reason: e.to_string(),
            })?;
    let parsed =
        ParsedCreateDeploymentRequest::parse(request, &state.args.dataplane.default_region)?;

    let command = CreateDeploymentCommand::new(
        organisation_id,
        DeploymentName(parsed.name),
        parsed.kind,
        parsed.version,
        created_by,
        parsed.environment,
        parsed.region,
        parsed.offer,
    );

    let deployment = state.service.create_deployment(identity, command).await?;

    // Best-effort and never awaited: a deployment is created whether or not
    // it can be given a DNS record yet, and a data plane with no address yet
    // is caught up by the reconciliation sweep instead.
    tokio::spawn(crate::dns::reconcile_deployment(
        state.clone(),
        deployment.clone(),
    ));

    Ok(Response::Created(CreateDeploymentResponse {
        data: deployment,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_deployment_rejects_invalid_kind() {
        assert!(matches!(
            parse(CreateDeploymentRequest {
                kind: "invalid".to_string(),
                ..a_request()
            }),
            Err(ApiError::BadRequest { .. })
        ));
    }

    #[tokio::test]
    async fn create_deployment_rejects_an_environment_nobody_named() {
        assert!(matches!(
            parse(CreateDeploymentRequest {
                environment: "preprod".to_string(),
                ..a_request()
            }),
            Err(ApiError::BadRequest { .. })
        ));
    }

    #[tokio::test]
    async fn create_deployment_rejects_an_offer_nobody_sells() {
        assert!(matches!(
            parse(CreateDeploymentRequest {
                offer: "enterprise-plus".to_string(),
                ..a_request()
            }),
            Err(ApiError::BadRequest { .. })
        ));
    }

    /// A region is the fleet seen from outside, and choosing one is choosing
    /// which cluster serves you. Refused rather than dropped: a caller that
    /// asked for one country and was quietly given another would find out
    /// from where their data is.
    #[tokio::test]
    async fn a_request_that_names_a_region_is_refused_rather_than_ignored() {
        let Err(ApiError::BadRequest { reason }) = parse(CreateDeploymentRequest {
            region: Some("fr-par".to_string()),
            ..a_request()
        }) else {
            panic!("a caller placed their own deployment");
        };
        assert!(
            reason.contains("region"),
            "and it says which field: {reason}"
        );
    }

    #[tokio::test]
    async fn a_request_that_names_none_runs_where_the_installation_says() {
        assert_eq!(
            parse(a_request()).expect("a valid request").region.as_str(),
            "somewhere-else"
        );
    }

    /// What the offer carries is not something the request can contradict,
    /// because there is nowhere to put a contradiction.
    #[tokio::test]
    async fn the_size_and_the_isolation_are_read_from_the_offer() {
        let parsed = parse(CreateDeploymentRequest {
            offer: "private".to_string(),
            ..a_request()
        })
        .expect("a valid request");

        assert_eq!(parsed.offer, Offer::Private);
        assert_eq!(
            parsed.offer.mode(),
            aether_core::dataplane::value_objects::DataPlaneMode::Dedicated
        );
    }

    fn a_request() -> CreateDeploymentRequest {
        CreateDeploymentRequest {
            name: "deployment".to_string(),
            kind: "keycloak".to_string(),
            version: "1.0.0".to_string(),
            environment: "production".to_string(),
            region: None,
            offer: "standard".to_string(),
        }
    }

    fn parse(request: CreateDeploymentRequest) -> Result<ParsedCreateDeploymentRequest, ApiError> {
        ParsedCreateDeploymentRequest::parse(request, "somewhere-else")
    }
}
