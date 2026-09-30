use aether_auth::Identity;
use aether_core::{
    action::{Action, ActionId},
    dataplane::value_objects::DataPlaneId,
    dataplane_upgrade::{UpgradeComponent, UpgradeStrategy},
    dataplane_upgrade_request::{
        DataplaneUpgradeError, DataplaneUpgradeRequest, DataplaneUpgrades, RequestedUpgrade,
    },
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath)]
#[typed_path("/platform/dataplanes/upgrade")]
pub struct DataplaneUpgradeRoute;

#[derive(TypedPath)]
#[typed_path("/platform/dataplanes/upgrades")]
pub struct DataplaneUpgradesRoute;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Every {
    All,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum DataplaneSelection {
    Every(Every),
    Only(Vec<DataPlaneId>),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum ComponentSelection {
    Every(Every),
    Only(Vec<String>),
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpgradeDataplanesRequest {
    /// The version every targeted data plane is upgraded to.
    pub target_version: String,

    /// The ids of the data planes to upgrade, or `"all"`.
    pub dataplane_ids: DataplaneSelection,

    /// Any of `Herald`, `Genesis`, `Operator` and `All`, or `"all"`.
    pub components: ComponentSelection,

    /// `rolling` or `canary`, `rolling` when absent.
    pub strategy: Option<String>,

    /// How many instances may be down at once, 1 when absent.
    pub max_unavailable: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct RequestedUpgradeItem {
    pub dataplane_id: DataPlaneId,
    pub action_id: ActionId,
    /// True when an upgrade to the same version was already open for this
    /// data plane, and nothing new was created for it.
    pub already_requested: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct RequestedUpgradesResponse {
    pub data: Vec<RequestedUpgradeItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct DataplaneUpgradeActions {
    pub dataplane_id: DataPlaneId,
    /// Oldest first. Each action's `status` is where the control plane's
    /// hand-off stands: pending, leased or pulled (claimed), published or
    /// failed.
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct DataplaneUpgradesResponse {
    pub data: Vec<DataplaneUpgradeActions>,
}

#[utoipa::path(
    post,
    path = "/dataplanes/upgrade",
    summary = "upgrade data planes",
    tag = "platform",
    description = "Records one dataplane.upgrade action per targeted data plane, for its Herald \
                   to hand to the operator. All or none: if any target is invalid, unknown, \
                   below DATAPLANE_UPGRADE_MIN_VERSION (or reporting no version, or the minimum \
                   being unset) or already being upgraded to another version, nothing is \
                   created. Asking again for the version a data plane is already being upgraded \
                   to returns the existing action. Requires operate_fleet.",
    request_body = UpgradeDataplanesRequest,
    responses(
        (status = 202, description = "The upgrade actions, one per data plane", body = RequestedUpgradesResponse),
        (status = 400, description = "The request is not a valid upgrade", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller does not hold operate_fleet", body = ApiError),
        (status = 404, description = "A named data plane does not exist", body = ApiError),
        (status = 409, description = "A data plane cannot be upgraded now", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn upgrade_dataplanes_handler(
    _: DataplaneUpgradeRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<UpgradeDataplanesRequest>,
) -> Result<Response<RequestedUpgradesResponse>, ApiError> {
    let request = into_request(request)?;

    let requested = state
        .service
        .request_dataplane_upgrade(
            identity,
            state.args.dataplane.upgrade_min_version.as_ref(),
            request,
        )
        .await
        .map_err(upgrade_error)?;

    Ok(Response::Accepted(RequestedUpgradesResponse {
        data: requested.iter().map(item).collect(),
    }))
}

/// The upgrade actions of every data plane that has any.
///
/// Only the hand-off is visible here. The phase and progress of the upgrade
/// itself live in the cluster, on the resource the operator reconciles, and
/// the control plane is never told them.
#[utoipa::path(
    get,
    path = "/dataplanes/upgrades",
    summary = "list data plane upgrades",
    tag = "platform",
    description = "The dataplane.upgrade actions recorded for each data plane, with their \
                   payload and hand-off status. The phase and progress of the upgrade itself \
                   live in the cluster and are not visible to the control plane. Requires \
                   view_estate.",
    responses(
        (status = 200, description = "Upgrade actions per data plane", body = DataplaneUpgradesResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller does not hold view_estate", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_dataplane_upgrades_handler(
    _: DataplaneUpgradesRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<DataplaneUpgradesResponse>, ApiError> {
    let listed = state.service.list_dataplane_upgrades(identity).await?;

    Ok(Response::OK(DataplaneUpgradesResponse {
        data: listed.into_iter().map(actions_of).collect(),
    }))
}

fn item(requested: &RequestedUpgrade) -> RequestedUpgradeItem {
    RequestedUpgradeItem {
        dataplane_id: requested.dataplane_id,
        action_id: requested.action.id,
        already_requested: requested.already_requested,
    }
}

fn actions_of(upgrades: DataplaneUpgrades) -> DataplaneUpgradeActions {
    DataplaneUpgradeActions {
        dataplane_id: upgrades.dataplane_id,
        actions: upgrades.actions,
    }
}

fn into_request(request: UpgradeDataplanesRequest) -> Result<DataplaneUpgradeRequest, ApiError> {
    let components = match request.components {
        ComponentSelection::Every(_) => vec![UpgradeComponent::All],
        ComponentSelection::Only(names) => names
            .iter()
            .map(|name| component(name))
            .collect::<Result<_, _>>()?,
    };

    Ok(DataplaneUpgradeRequest {
        target_version: request.target_version,
        dataplanes: match request.dataplane_ids {
            DataplaneSelection::Every(_) => None,
            DataplaneSelection::Only(ids) => Some(ids),
        },
        components,
        strategy: strategy(request.strategy.as_deref())?,
        max_unavailable: request.max_unavailable.unwrap_or(1),
    })
}

fn component(name: &str) -> Result<UpgradeComponent, ApiError> {
    match name {
        "Herald" => Ok(UpgradeComponent::Herald),
        "Genesis" => Ok(UpgradeComponent::Genesis),
        "Operator" => Ok(UpgradeComponent::Operator),
        "All" => Ok(UpgradeComponent::All),
        other => Err(ApiError::BadRequest {
            reason: format!(
                "'{other}' is not a component: expected Herald, Genesis, Operator or All"
            ),
        }),
    }
}

fn strategy(name: Option<&str>) -> Result<UpgradeStrategy, ApiError> {
    match name {
        None | Some("rolling") => Ok(UpgradeStrategy::Rolling),
        Some("canary") => Ok(UpgradeStrategy::Canary),
        Some(other) => Err(ApiError::BadRequest {
            reason: format!("'{other}' is not a strategy: expected rolling or canary"),
        }),
    }
}

fn upgrade_error(error: DataplaneUpgradeError) -> ApiError {
    match error {
        DataplaneUpgradeError::Invalid(_) | DataplaneUpgradeError::BadVersion(_) => {
            ApiError::BadRequest {
                reason: error.to_string(),
            }
        }
        DataplaneUpgradeError::UnknownDataplane(_) => ApiError::NotFound {
            reason: error.to_string(),
        },
        DataplaneUpgradeError::NoMinimumConfigured { .. }
        | DataplaneUpgradeError::HeraldTooOld { .. }
        | DataplaneUpgradeError::AlreadyUpgrading { .. } => ApiError::Conflict {
            reason: error.to_string(),
        },
        DataplaneUpgradeError::Core(core) => ApiError::from(core),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{app_state, user_identity};
    use aether_core::dataplane_upgrade::InvalidDataplaneUpgrade;
    use serde_json::json;
    use uuid::Uuid;

    fn request(body: serde_json::Value) -> UpgradeDataplanesRequest {
        serde_json::from_value(body).unwrap()
    }

    async fn post(
        body: serde_json::Value,
    ) -> Result<Response<RequestedUpgradesResponse>, ApiError> {
        upgrade_dataplanes_handler(
            DataplaneUpgradeRoute,
            State(app_state()),
            Extension(user_identity("operator-1")),
            Json(request(body)),
        )
        .await
    }

    #[test]
    fn the_request_takes_ids_or_all_and_defaults_strategy_and_max_unavailable() {
        let id = Uuid::new_v4();

        let named = into_request(request(json!({
            "target_version": "26.1.0",
            "dataplane_ids": [id],
            "components": ["Herald", "Genesis"],
        })))
        .unwrap();
        let every = into_request(request(json!({
            "target_version": "26.1.0",
            "dataplane_ids": "all",
            "components": "all",
            "strategy": "canary",
            "max_unavailable": 3,
        })))
        .unwrap();

        assert_eq!(named.dataplanes, Some(vec![DataPlaneId(id)]));
        assert_eq!(
            named.components,
            vec![UpgradeComponent::Herald, UpgradeComponent::Genesis]
        );
        assert_eq!(named.strategy, UpgradeStrategy::Rolling);
        assert_eq!(named.max_unavailable, 1);
        assert_eq!(every.dataplanes, None);
        assert_eq!(every.components, vec![UpgradeComponent::All]);
        assert_eq!(every.strategy, UpgradeStrategy::Canary);
        assert_eq!(every.max_unavailable, 3);
    }

    #[tokio::test]
    async fn an_unknown_strategy_is_refused_before_anything_is_recorded() {
        let result = post(json!({
            "target_version": "26.1.0",
            "dataplane_ids": "all",
            "components": "all",
            "strategy": "yolo",
        }))
        .await;

        assert!(matches!(result, Err(ApiError::BadRequest { .. })));
    }

    #[tokio::test]
    async fn an_unknown_component_is_refused_before_anything_is_recorded() {
        let result = post(json!({
            "target_version": "26.1.0",
            "dataplane_ids": "all",
            "components": ["Kraken"],
        }))
        .await;

        assert!(matches!(result, Err(ApiError::BadRequest { .. })));
    }

    #[tokio::test]
    async fn a_valid_request_reaches_the_service() {
        let result = post(json!({
            "target_version": "26.1.0",
            "dataplane_ids": "all",
            "components": "all",
        }))
        .await;

        assert!(matches!(result, Err(ApiError::Unknown { .. })));
    }

    #[tokio::test]
    async fn listing_maps_service_error() {
        let result = list_dataplane_upgrades_handler(
            DataplaneUpgradesRoute,
            State(app_state()),
            Extension(user_identity("operator-1")),
        )
        .await;

        assert!(matches!(result, Err(ApiError::Unknown { .. })));
    }

    #[test]
    fn errors_map_to_their_status() {
        let id = DataPlaneId(Uuid::nil());

        assert!(matches!(
            upgrade_error(DataplaneUpgradeError::Invalid(
                InvalidDataplaneUpgrade::NoComponents
            )),
            ApiError::BadRequest { .. }
        ));
        assert!(matches!(
            upgrade_error(DataplaneUpgradeError::UnknownDataplane(id)),
            ApiError::NotFound { .. }
        ));
        assert!(matches!(
            upgrade_error(DataplaneUpgradeError::NoMinimumConfigured { dataplane: id }),
            ApiError::Conflict { reason } if reason.contains(&id.0.to_string())
        ));
        assert!(matches!(
            upgrade_error(DataplaneUpgradeError::AlreadyUpgrading {
                dataplane: id,
                running: None
            }),
            ApiError::Conflict { .. }
        ));
    }
}
