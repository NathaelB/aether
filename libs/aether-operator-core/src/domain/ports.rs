use std::future::Future;

use aether_crds::v1alpha::identity_instance::{IdentityInstance, IdentityInstanceStatus};

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

    fn ingress_ready(
        &self,
        instance: &IdentityInstance,
    ) -> impl Future<Output = Result<bool, OperatorError>> + Send;

    fn upgrade_in_progress(
        &self,
        instance: &IdentityInstance,
    ) -> impl Future<Output = Result<bool, OperatorError>> + Send;
}
