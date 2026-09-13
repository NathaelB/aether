use aether_auth::Identity;
use aether_core::{
    dataplane::value_objects::{DataPlaneId, Region},
    deployments::{DeploymentId, DeploymentStatus},
    organisation::OrganisationId,
    platform::{EstateDeployment, EstateQuery, ports::PlatformService},
};
use axum::{
    Extension,
    extract::{Query, State},
};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath)]
#[typed_path("/platform/deployments")]
pub struct EstateDeploymentsRoute;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct EstateDeploymentsQuery {
    /// Only this organisation's deployments.
    pub organisation_id: Option<Uuid>,

    /// Only what sits on this data plane, which is the question asked before
    /// touching a cluster.
    pub dataplane_id: Option<Uuid>,

    pub region: Option<String>,

    /// One of the deployment statuses. A status nobody uses is refused rather
    /// than matched against nothing, which would read as an empty estate.
    pub status: Option<String>,

    pub limit: Option<usize>,

    /// The last deployment of the previous page.
    pub cursor: Option<Uuid>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct EstateDeploymentsResponse {
    pub data: Vec<EstateDeployment>,

    /// Absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<DeploymentId>,
}

#[utoipa::path(
    get,
    path = "/deployments",
    summary = "list every deployment on the installation",
    tag = "platform",
    description = "Whatever organisation owns them. Each row carries the organisation, the \
                   data plane and the region, so a screen showing two hundred deployments \
                   does not open two hundred requests to draw one page.",
    params(EstateDeploymentsQuery),
    responses(
        (status = 200, description = "One page of the estate", body = EstateDeploymentsResponse),
        (status = 400, description = "The request does not describe a page", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "Running the installation is a separate right", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_estate_deployments_handler(
    _: EstateDeploymentsRoute,
    Query(query): Query<EstateDeploymentsQuery>,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<EstateDeploymentsResponse>, ApiError> {
    let page = state
        .service
        .list_estate_deployments(identity, query.into_query()?)
        .await?;

    Ok(Response::OK(EstateDeploymentsResponse {
        data: page.deployments,
        next_cursor: page.next_cursor,
    }))
}

impl EstateDeploymentsQuery {
    /// Reads the request as a page of the estate, naming whichever parameter
    /// could not be read rather than refusing as a whole.
    fn into_query(self) -> Result<EstateQuery, ApiError> {
        let refused = |reason: String| ApiError::BadRequest { reason };

        let status = self
            .status
            .as_deref()
            .map(DeploymentStatus::try_from)
            .transpose()
            .map_err(|_| {
                refused(format!(
                    "'{}' is not a deployment status",
                    self.status.clone().unwrap_or_default()
                ))
            })?;

        Ok(EstateQuery::new(self.limit, self.cursor.map(DeploymentId))
            .map_err(refused)?
            .in_organisation(self.organisation_id.map(OrganisationId))
            .on_dataplane(self.dataplane_id.map(DataPlaneId))
            .in_region(self.region.map(Region::new))
            .with_status(status))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query() -> EstateDeploymentsQuery {
        EstateDeploymentsQuery {
            organisation_id: None,
            dataplane_id: None,
            region: None,
            status: None,
            limit: None,
            cursor: None,
        }
    }

    /// A status nobody uses would otherwise match no row, and an empty page
    /// says the estate is empty rather than that the filter was a typo.
    #[test]
    fn a_status_nobody_uses_is_refused_rather_than_matched_against_nothing() {
        let refused = EstateDeploymentsQuery {
            status: Some("halfway".to_string()),
            ..query()
        }
        .into_query()
        .expect_err("a status nobody uses was accepted");

        let ApiError::BadRequest { reason } = refused else {
            panic!("the refusal was not a bad request");
        };
        assert!(reason.contains("halfway"), "got {reason}");
    }

    #[test]
    fn a_page_beyond_the_ceiling_is_refused() {
        assert!(
            EstateDeploymentsQuery {
                limit: Some(aether_core::platform::MAX_PAGE + 1),
                ..query()
            }
            .into_query()
            .is_err()
        );
    }

    /// Every filter is optional, and none of them defaults to something
    /// narrower than the whole estate.
    #[test]
    fn a_request_with_no_filters_asks_about_the_whole_estate() {
        let whole = query().into_query().expect("a valid request");

        assert!(whole.organisation.is_none());
        assert!(whole.dataplane.is_none());
        assert!(whole.region.is_none());
        assert!(whole.status.is_none());
    }

    #[test]
    fn every_filter_reaches_the_query() {
        let narrowed = EstateDeploymentsQuery {
            organisation_id: Some(Uuid::from_u128(1)),
            dataplane_id: Some(Uuid::from_u128(2)),
            region: Some("fr-par".to_string()),
            status: Some("successful".to_string()),
            limit: Some(10),
            cursor: Some(Uuid::from_u128(3)),
        }
        .into_query()
        .expect("a valid request");

        assert_eq!(
            narrowed.organisation,
            Some(OrganisationId(Uuid::from_u128(1)))
        );
        assert_eq!(narrowed.dataplane, Some(DataPlaneId(Uuid::from_u128(2))));
        assert_eq!(narrowed.region.as_ref().map(Region::as_str), Some("fr-par"));
        assert_eq!(narrowed.status, Some(DeploymentStatus::Successful));
        assert_eq!(narrowed.limit, 10);
        assert_eq!(narrowed.cursor, Some(DeploymentId(Uuid::from_u128(3))));
    }
}
