use std::future::Future;

use aether_crds::v1alpha::identity_dataplane_upgrade::{
    IdentityDataplaneUpgrade, IdentityDataplaneUpgradeStatus,
};
use aether_crds::v1alpha::identity_instance::{IdentityInstance, IdentityInstanceStatus};

use crate::domain::dataplane_upgrade::{ComponentVersions, DataplaneComponentKind};
use crate::domain::identity_instance::theme::ThemeError;
use crate::domain::{OperatorError, ReconcileOutcome};

pub trait IdentityInstanceService: Send + Sync {
    fn reconcile(
        &self,
        instance: IdentityInstance,
    ) -> impl Future<Output = Result<ReconcileOutcome, OperatorError>> + Send;
}

#[cfg_attr(test, mockall::automock)]
pub trait IdentityInstanceRepository: Send + Sync {
    fn patch_status(
        &self,
        instance: &IdentityInstance,
        status: IdentityInstanceStatus,
    ) -> impl Future<Output = Result<IdentityInstance, OperatorError>> + Send;
}

#[cfg_attr(test, mockall::automock)]
pub trait InstanceThemePort: Send + Sync {
    fn put_theme(
        &self,
        instance: &IdentityInstance,
        config: &serde_json::Value,
    ) -> impl Future<Output = Result<(), ThemeError>> + Send;
}

#[cfg_attr(test, mockall::automock)]
pub trait IdentityInstanceDeployer: Send + Sync {
    fn ensure_provider_resources(
        &self,
        instance: &IdentityInstance,
    ) -> impl Future<Output = Result<(), OperatorError>> + Send;

    fn cleanup_provider_resources(
        &self,
        instance: &IdentityInstance,
    ) -> impl Future<Output = Result<(), OperatorError>> + Send;

    fn provider_ready(
        &self,
        instance: &IdentityInstance,
    ) -> impl Future<Output = Result<bool, OperatorError>> + Send;

    fn database_ready(
        &self,
        instance: &IdentityInstance,
    ) -> impl Future<Output = Result<bool, OperatorError>> + Send;

    /// Whether the edge is actually serving this instance.
    ///
    /// Named for what it answers rather than for the object that answers it:
    /// an Ingress yesterday, an HTTPRoute today, and the reconcile loop has
    /// no business knowing which.
    fn edge_ready(
        &self,
        instance: &IdentityInstance,
    ) -> impl Future<Output = Result<bool, OperatorError>> + Send;

    fn upgrade_in_progress(
        &self,
        instance: &IdentityInstance,
    ) -> impl Future<Output = Result<bool, OperatorError>> + Send;
}

#[cfg_attr(test, mockall::automock)]
pub trait DataplaneUpgradeRepository: Send + Sync {
    fn get(
        &self,
        name: &str,
        namespace: &str,
    ) -> impl Future<Output = Result<Option<IdentityDataplaneUpgrade>, OperatorError>> + Send;

    fn patch_status(
        &self,
        name: &str,
        namespace: &str,
        status: IdentityDataplaneUpgradeStatus,
    ) -> impl Future<Output = Result<(), OperatorError>> + Send;
}

#[cfg_attr(test, mockall::automock)]
pub trait DataplaneUpgradeDeployer: Send + Sync {
    fn current_versions(
        &self,
        namespace: &str,
    ) -> impl Future<Output = Result<ComponentVersions, OperatorError>> + Send;

    fn set_component_version(
        &self,
        namespace: &str,
        component: DataplaneComponentKind,
        version: &str,
    ) -> impl Future<Output = Result<(), OperatorError>> + Send;

    fn component_ready(
        &self,
        namespace: &str,
        component: DataplaneComponentKind,
    ) -> impl Future<Output = Result<bool, OperatorError>> + Send;
}
