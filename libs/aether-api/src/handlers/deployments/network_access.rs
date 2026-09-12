use aether_auth::Identity;
use aether_core::{
    deployments::{
        Deployment, DeploymentId,
        network::{Cidr, NetworkAccess},
        ports::NetworkAccessService,
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
#[typed_path("/organisations/{organisation_id}/deployments/{deployment_id}/network-access")]
pub struct NetworkAccessRoute {
    pub organisation_id: Uuid,
    pub deployment_id: Uuid,
}

#[derive(Deserialize, ToSchema)]
pub struct SetNetworkAccessRequest {
    /// The ranges that may reach this deployment, in CIDR notation.
    ///
    /// An empty list is open, not closed. Somebody removing the last entry in
    /// a form means "stop restricting this", and reading it as "restrict this
    /// to nobody" would take their identity provider off the air.
    #[serde(default)]
    pub allowed_cidrs: Vec<String>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct NetworkAccessResponse {
    pub data: NetworkAccess,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct SetNetworkAccessResponse {
    pub data: Deployment,
}

/// Names the value that could not be read, rather than which element of an
/// array it was. A caller looking at their own form needs to know which line
/// to fix, and an index into a list they may have reordered does not say.
fn parse_ranges(raw: Vec<String>) -> Result<Vec<Cidr>, ApiError> {
    raw.into_iter()
        .map(|value| {
            value.parse::<Cidr>().map_err(|e| ApiError::BadRequest {
                reason: e.to_string(),
            })
        })
        .collect()
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/deployments/{deployment_id}/network-access",
    summary = "read which source addresses may reach a deployment",
    tag = "deployments",
    description = "Open means reachable from anywhere, which is what a deployment is until somebody restricts it.",
    params(NetworkAccessRoute),
    responses(
        (status = 200, description = "Who may reach this deployment", body = NetworkAccessResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not view this deployment", body = ApiError),
        (status = 404, description = "No such deployment", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_network_access_handler(
    NetworkAccessRoute {
        organisation_id,
        deployment_id,
    }: NetworkAccessRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<NetworkAccessResponse>, ApiError> {
    let access = state
        .service
        .network_access(
            identity,
            OrganisationId(organisation_id),
            DeploymentId(deployment_id),
        )
        .await?;

    Ok(Response::OK(NetworkAccessResponse { data: access }))
}

#[utoipa::path(
    put,
    path = "/{organisation_id}/deployments/{deployment_id}/network-access",
    summary = "replace which source addresses may reach a deployment",
    tag = "deployments",
    description = "Replaces the whole rule rather than adding or removing one range. An empty list means open: clearing the last entry stops the restriction rather than restricting to nobody.",
    params(NetworkAccessRoute),
    request_body = SetNetworkAccessRequest,
    responses(
        (status = 200, description = "The deployment with its new rule", body = SetNetworkAccessResponse),
        (status = 400, description = "One of the ranges is not a CIDR", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not change this deployment", body = ApiError),
        (status = 404, description = "No such deployment", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn set_network_access_handler(
    NetworkAccessRoute {
        organisation_id,
        deployment_id,
    }: NetworkAccessRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<SetNetworkAccessRequest>,
) -> Result<Response<SetNetworkAccessResponse>, ApiError> {
    let ranges = parse_ranges(request.allowed_cidrs)?;
    let access = NetworkAccess::from_ranges(ranges).map_err(|e| ApiError::BadRequest {
        reason: e.to_string(),
    })?;

    let deployment = state
        .service
        .set_network_access(
            identity,
            OrganisationId(organisation_id),
            DeploymentId(deployment_id),
            access,
        )
        .await?;

    Ok(Response::OK(SetNetworkAccessResponse { data: deployment }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 422 from the body deserialiser says a field was the wrong type. This
    /// says which value was not a range, which is the only thing the person
    /// who typed it needs.
    #[test]
    fn a_range_that_cannot_be_read_is_named_in_the_refusal() {
        let error = parse_ranges(vec!["10.0.0.0/8".into(), "not-a-range".into()])
            .expect_err("the second one");

        let ApiError::BadRequest { reason } = error else {
            panic!("expected a bad request");
        };
        assert!(reason.contains("not-a-range"), "{reason}");
    }

    /// The trap the whole slice exists to close, at the last boundary before
    /// the domain. Clearing every entry is going back to open.
    #[test]
    fn an_empty_list_is_open_rather_than_closed() {
        let access =
            NetworkAccess::from_ranges(parse_ranges(Vec::new()).expect("nothing")).expect("open");

        assert_eq!(access, NetworkAccess::Open);
    }

    #[test]
    fn ranges_that_read_become_the_rule() {
        let access =
            NetworkAccess::from_ranges(parse_ranges(vec!["203.0.113.0/24".into()]).expect("one"))
                .expect("restricted");

        assert!(!access.is_open());
        assert_eq!(access.ranges()[0].to_string(), "203.0.113.0/24");
    }

    /// Host bits are a typo for the network containing that address, and the
    /// message has to carry the suggestion out to the caller rather than stop
    /// at the domain.
    #[test]
    fn a_range_with_host_bits_suggests_what_was_meant() {
        let error = parse_ranges(vec!["203.0.113.9/24".into()]).expect_err("host bits");

        let ApiError::BadRequest { reason } = error else {
            panic!("expected a bad request");
        };
        assert!(reason.contains("203.0.113.0/24"), "{reason}");
    }
}
