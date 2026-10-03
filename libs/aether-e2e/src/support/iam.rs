use std::sync::{Arc, Mutex};

use aether_auth::Identity;
use aether_crds::v1alpha::identity_instance::{Branding as ResourceBranding, IamConfig};
use aether_domain::CoreError;
use aether_domain::action::Action;
use aether_domain::dataplane::value_objects::{DataPlaneId, DeploymentResources};
use aether_domain::deployments::environment::Environment;
use aether_domain::deployments::network::NetworkAccess;
use aether_domain::deployments::ports::DeploymentRepository;
use aether_domain::deployments::{
    Deployment, DeploymentId, DeploymentKind, DeploymentName, DeploymentStatus,
};
use aether_domain::iam_settings::branding::{Branding, BrandingInput};
use aether_domain::iam_settings::ports::IamSettingsPolicy;
use aether_domain::iam_settings::service::payload;
use aether_domain::iam_settings::{IAM_SETTINGS_ACTION_TYPE, IamSettings};
use aether_domain::organisation::commands::CreateOrganisationData;
use aether_domain::organisation::member::Member;
use aether_domain::organisation::ports::OrganisationRepository;
use aether_domain::organisation::value_objects::{
    OrganisationName, OrganisationSlug, OrganisationStatus, Plan,
};
use aether_domain::organisation::{Organisation, OrganisationId};
use aether_domain::role::RoleId;
use aether_domain::user::UserId;
use aether_domain::version::Version;
use chrono::Utc;
use genesis_core::domain::entities::action_event::ActionEvent;
use genesis_core::domain::entities::iam_settings_payload::{
    Branding as GenesisBranding, IamSettingsPayloadV1,
};
use genesis_core::domain::entities::identity_instance::{
    DesiredIdentityInstance, IdentityInstanceRef,
};
use genesis_core::domain::error::GenesisError;
use genesis_core::domain::ports::{BoxFuture, IdentityInstancePort};
use genesis_core::infrastructure::kubernetes::identity_instance::iam_patch;
use serde_json::Value;
use uuid::Uuid;

pub const ORGANISATION: Uuid = Uuid::from_u128(1);
pub const DEPLOYMENT: Uuid = Uuid::from_u128(2);
pub const NAMESPACE: &str = "production-auth";

fn unused() -> CoreError {
    CoreError::DatabaseError {
        message: "not used by these scenarios".to_string(),
    }
}

pub fn deployment_of(organisation: Uuid) -> Deployment {
    let at = Utc::now();
    Deployment {
        id: DeploymentId(DEPLOYMENT),
        organisation_id: OrganisationId(organisation),
        dataplane_id: DataPlaneId(Uuid::from_u128(3)),
        name: DeploymentName("auth".to_string()),
        kind: DeploymentKind::Ferriskey,
        version: Version::new(26, 0, 1),
        status: DeploymentStatus::Successful,
        namespace: NAMESPACE.to_string(),
        environment: Environment::Development,
        offer: None,
        restored_from: None,
        resources: DeploymentResources::DEFAULT,
        created_by: UserId(Uuid::from_u128(4)),
        created_at: at,
        updated_at: at,
        deployed_at: None,
        deleted_at: None,
        auto_upgrade: Default::default(),
        maintenance_window: None,
        network_access: NetworkAccess::Open,
        last_verified_restore_at: None,
        last_restore_drill_seconds: None,
        log_shipping_enabled: false,
        iam_settings: Default::default(),
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryDeployments {
    rows: Arc<Mutex<Vec<Deployment>>>,
    updates: Arc<Mutex<usize>>,
}

impl InMemoryDeployments {
    pub fn holding(deployment: Deployment) -> Self {
        let held = Self::default();
        held.rows.lock().expect("not poisoned").push(deployment);
        held
    }

    pub fn branding(&self) -> Option<Branding> {
        self.rows
            .lock()
            .expect("not poisoned")
            .first()
            .and_then(|deployment| deployment.iam_settings.branding.clone())
    }

    pub fn updates(&self) -> usize {
        *self.updates.lock().expect("not poisoned")
    }
}

impl DeploymentRepository for InMemoryDeployments {
    async fn purge_deleted(&self, _: chrono::DateTime<Utc>) -> Result<u64, CoreError> {
        Ok(0)
    }

    async fn insert(&self, _: Deployment) -> Result<(), CoreError> {
        Err(unused())
    }

    async fn get_by_id(&self, id: DeploymentId) -> Result<Option<Deployment>, CoreError> {
        Ok(self
            .rows
            .lock()
            .expect("not poisoned")
            .iter()
            .find(|deployment| deployment.id == id)
            .cloned())
    }

    async fn list_by_organisation(&self, _: OrganisationId) -> Result<Vec<Deployment>, CoreError> {
        Ok(Vec::new())
    }

    async fn update(&self, deployment: Deployment) -> Result<(), CoreError> {
        let mut rows = self.rows.lock().expect("not poisoned");
        if let Some(row) = rows.iter_mut().find(|row| row.id == deployment.id) {
            *row = deployment;
        }
        *self.updates.lock().expect("not poisoned") += 1;
        Ok(())
    }

    async fn delete(&self, _: DeploymentId) -> Result<(), CoreError> {
        Err(unused())
    }

    async fn list_by_dataplane(&self, _: &DataPlaneId) -> Result<Vec<Deployment>, CoreError> {
        Ok(Vec::new())
    }

    async fn list_all_live(&self) -> Result<Vec<Deployment>, CoreError> {
        Ok(Vec::new())
    }

    async fn count_by_version(&self, _: &DeploymentKind) -> Result<Vec<(Version, u64)>, CoreError> {
        Ok(Vec::new())
    }
}

#[derive(Debug, Clone)]
pub struct OneOrganisation(Plan);

impl OneOrganisation {
    pub fn on(plan: Plan) -> Self {
        Self(plan)
    }
}

impl OrganisationRepository for OneOrganisation {
    async fn create(&self, _: CreateOrganisationData) -> Result<Organisation, CoreError> {
        Err(unused())
    }

    async fn insert_member(&self, _: &OrganisationId, _: &UserId) -> Result<(), CoreError> {
        Err(unused())
    }

    async fn find_by_id(&self, id: &OrganisationId) -> Result<Option<Organisation>, CoreError> {
        let mut organisation = Organisation::new(
            OrganisationName::new("FerrisLabs")?,
            OrganisationSlug::new("ferrislabs")?,
            UserId(Uuid::from_u128(7)),
            self.0,
        );
        organisation.id = *id;
        Ok(Some(organisation))
    }

    async fn find_by_slug(&self, _: &OrganisationSlug) -> Result<Option<Organisation>, CoreError> {
        Ok(None)
    }

    async fn find_by_owner(&self, _: &UserId) -> Result<Vec<Organisation>, CoreError> {
        Ok(Vec::new())
    }

    async fn find_member(
        &self,
        _: &OrganisationId,
        _: &UserId,
    ) -> Result<Option<Member>, CoreError> {
        Ok(None)
    }

    async fn list_members(&self, _: &OrganisationId) -> Result<Vec<Member>, CoreError> {
        Ok(Vec::new())
    }

    async fn set_member_roles(
        &self,
        _: &OrganisationId,
        _: &UserId,
        _: &[RoleId],
    ) -> Result<(), CoreError> {
        Err(unused())
    }

    async fn remove_member(&self, _: &OrganisationId, _: &UserId) -> Result<(), CoreError> {
        Err(unused())
    }

    async fn find_by_member(&self, _: &UserId) -> Result<Vec<Organisation>, CoreError> {
        Ok(Vec::new())
    }

    async fn list(
        &self,
        _: Option<OrganisationStatus>,
        _: usize,
        _: usize,
    ) -> Result<Vec<Organisation>, CoreError> {
        Ok(Vec::new())
    }

    async fn update(&self, _: Organisation) -> Result<Organisation, CoreError> {
        Err(unused())
    }

    async fn delete(&self, _: &OrganisationId) -> Result<(), CoreError> {
        Err(unused())
    }

    async fn slug_exists(&self, _: &OrganisationSlug) -> Result<bool, CoreError> {
        Ok(false)
    }

    async fn count(&self) -> Result<usize, CoreError> {
        Ok(0)
    }

    async fn count_by_status(&self, _: OrganisationStatus) -> Result<usize, CoreError> {
        Ok(0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InstanceRights {
    may_manage: bool,
}

impl InstanceRights {
    pub fn to_manage() -> Self {
        Self { may_manage: true }
    }

    pub fn to_view_only() -> Self {
        Self { may_manage: false }
    }
}

impl IamSettingsPolicy for InstanceRights {
    async fn can_view_iam_settings(&self, _: Identity, _: OrganisationId) -> Result<(), CoreError> {
        Ok(())
    }

    async fn can_change_iam_settings(
        &self,
        _: Identity,
        _: OrganisationId,
    ) -> Result<(), CoreError> {
        if self.may_manage {
            return Ok(());
        }
        Err(CoreError::PermissionDenied {
            reason: "insufficient permissions".to_string(),
        })
    }
}

#[derive(Debug, Default)]
pub struct SpyInstances {
    patches: Mutex<Vec<(IdentityInstanceRef, Value)>>,
}

impl SpyInstances {
    pub fn patches(&self) -> Vec<(IdentityInstanceRef, Value)> {
        self.patches.lock().expect("not poisoned").clone()
    }
}

impl IdentityInstancePort for SpyInstances {
    fn take_archive<'a>(
        &'a self,
        _: &'a IdentityInstanceRef,
        _: &'a str,
    ) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async { Ok(()) })
    }

    fn apply<'a>(
        &'a self,
        _: &'a DesiredIdentityInstance,
    ) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async { Ok(()) })
    }

    fn delete<'a>(&'a self, _: &'a IdentityInstanceRef) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async { Ok(()) })
    }

    fn set_allowed_cidrs<'a>(
        &'a self,
        _: &'a IdentityInstanceRef,
        _: Option<Vec<String>>,
    ) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async { Ok(()) })
    }

    fn set_iam<'a>(
        &'a self,
        reference: &'a IdentityInstanceRef,
        branding: Option<GenesisBranding>,
    ) -> BoxFuture<'a, Result<(), GenesisError>> {
        self.patches
            .lock()
            .expect("not poisoned")
            .push((reference.clone(), iam_patch(branding.as_ref())));
        Box::pin(async { Ok(()) })
    }

    fn is_ready<'a>(
        &'a self,
        _: &'a IdentityInstanceRef,
    ) -> BoxFuture<'a, Result<bool, GenesisError>> {
        Box::pin(async { Ok(true) })
    }

    fn database_uri<'a>(
        &'a self,
        _: &'a IdentityInstanceRef,
    ) -> BoxFuture<'a, Result<String, GenesisError>> {
        Box::pin(async { Ok(String::new()) })
    }

    fn delete_namespace<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async { Ok(()) })
    }
}

pub fn iam_settings_actions(actions: &[Action]) -> Vec<&Action> {
    actions
        .iter()
        .filter(|action| action.action_type.0 == IAM_SETTINGS_ACTION_TYPE)
        .collect()
}

pub fn event_of(action: &Action) -> ActionEvent {
    ActionEvent {
        action_id: action.id.0,
        deployment_id: action.deployment_id.map(|id| id.0),
        dataplane_id: action.dataplane_id.0,
        routing_key: action.action_type.0.clone(),
        version: action.version.0,
        payload: action.payload.data.clone(),
        occurred_at: action.metadata.created_at,
    }
}

pub fn branding_from(wire: &str) -> Result<Branding, serde_json::Error> {
    serde_json::from_str(wire)
}

pub fn valid_branding(input: BrandingInput) -> Branding {
    Branding::try_from(input).expect("a valid branding")
}

pub fn settings_of(branding: Option<Branding>) -> IamSettings {
    IamSettings { branding }
}

pub fn payload_for(branding: Option<Branding>) -> Value {
    let mut deployment = deployment_of(ORGANISATION);
    deployment.iam_settings = settings_of(branding);
    payload(&deployment)
}

pub fn accepted_by_genesis(payload: &Value) -> IamSettingsPayloadV1 {
    IamSettingsPayloadV1::from_value(payload).expect("accepted by Genesis")
}

pub fn spec_iam_of(patch: &Value) -> Option<IamConfig> {
    match &patch["spec"]["iam"] {
        Value::Null => None,
        iam => Some(serde_json::from_value(iam.clone()).expect("an iam config")),
    }
}

pub fn resource_branding_of(patch: &Value) -> Option<ResourceBranding> {
    spec_iam_of(patch).and_then(|iam| iam.branding)
}

pub fn resource_branding_after_genesis(branding: &Branding) -> Option<ResourceBranding> {
    let payload = payload_for(Some(branding.clone()));
    let parsed = accepted_by_genesis(&payload);
    resource_branding_of(&iam_patch(parsed.branding.as_ref()))
}

pub fn domain_colors(branding: &Branding) -> [Option<String>; 7] {
    let colors = &branding.colors;
    [
        &colors.primary,
        &colors.primary_text,
        &colors.links,
        &colors.page_background,
        &colors.widget_background,
        &colors.text,
        &colors.error,
    ]
    .map(|color| color.as_ref().map(|color| color.as_str().to_string()))
}

pub fn resource_colors(branding: &ResourceBranding) -> [Option<String>; 7] {
    match &branding.colors {
        None => Default::default(),
        Some(colors) => [
            &colors.primary,
            &colors.primary_text,
            &colors.links,
            &colors.page_background,
            &colors.widget_background,
            &colors.text,
            &colors.error,
        ]
        .map(|color| color.clone()),
    }
}
