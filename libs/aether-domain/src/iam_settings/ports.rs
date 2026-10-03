use std::future::Future;

use aether_auth::Identity;
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    CoreError,
    action::{ActionId, ActionStatus},
    deployments::DeploymentId,
    iam_settings::{IamSettings, branding::Branding},
    organisation::OrganisationId,
};

pub trait IamSettingsPolicy: Send + Sync {
    fn can_view_iam_settings(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn can_change_iam_settings(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct IamSettingsRequest {
    pub action_id: ActionId,
    pub status: ActionStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct IamSettingsState {
    pub branding: Option<Branding>,
    pub request: Option<IamSettingsRequest>,
}

pub trait IamSettingsService: Send + Sync {
    fn iam_settings(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> impl Future<Output = Result<IamSettingsState, CoreError>> + Send;

    fn set_iam_settings(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
        settings: IamSettings,
    ) -> impl Future<Output = Result<IamSettingsState, CoreError>> + Send;
}
