use aether_auth::Identity;
use aether_domain::{
    CoreError,
    deployments::DeploymentId,
    iam_settings::{
        ports::{IamSettingsPolicy, IamSettingsService, IamSettingsState},
        service::IamSettingsServiceImpl,
    },
    organisation::OrganisationId,
    role::ports::PermissionProvider,
};
use aether_macros::transactional;
use aether_permission::Permissions;
use aether_postgres::organisation::PostgresOrganisationRepository;

pub use aether_domain::iam_settings::{
    IAM_SETTINGS_ACTION_TYPE, IamSettings, branding, ports, service,
};

use crate::{AetherService, infrastructure::role::permissions_in, policy::PolicyContext};

struct InstanceRights<R> {
    permissions: R,
}

impl<R> IamSettingsPolicy for InstanceRights<R>
where
    R: PermissionProvider,
{
    async fn can_view_iam_settings(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> Result<(), CoreError> {
        let permissions = self
            .permissions
            .permissions_for_organisation(identity, organisation_id)
            .await?;

        PolicyContext::new(permissions).require_permission(Permissions::VIEW_INSTANCES)
    }

    async fn can_change_iam_settings(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> Result<(), CoreError> {
        let permissions = self
            .permissions
            .permissions_for_organisation(identity, organisation_id)
            .await?;

        PolicyContext::new(permissions).require_permission(Permissions::MANAGE_INSTANCES)
    }
}

impl IamSettingsService for AetherService {
    #[transactional(deployment, action)]
    async fn iam_settings(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<IamSettingsState, CoreError> {
        IamSettingsServiceImpl::new(
            deployment_repository,
            action_repository,
            PostgresOrganisationRepository::new(&tx),
            InstanceRights {
                permissions: permissions_in(&tx),
            },
        )
        .iam_settings(identity, organisation_id, deployment_id)
        .await
    }

    #[transactional(deployment, action)]
    async fn set_iam_settings(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
        settings: IamSettings,
    ) -> Result<IamSettingsState, CoreError> {
        IamSettingsServiceImpl::new(
            deployment_repository,
            action_repository,
            PostgresOrganisationRepository::new(&tx),
            InstanceRights {
                permissions: permissions_in(&tx),
            },
        )
        .set_iam_settings(identity, organisation_id, deployment_id, settings)
        .await
    }
}
