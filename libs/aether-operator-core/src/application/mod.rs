use std::sync::Arc;

use aether_crds::v1alpha::identity_instance::IdentityInstance;

use crate::domain::identity_instance::IdentityInstanceServiceImpl;
use crate::domain::ports::{
    IdentityInstanceDeployer, IdentityInstanceRepository, IdentityInstanceService,
};
use crate::domain::{OperatorError, ReconcileOutcome};

pub struct OperatorApplication<R, D> {
    pub identity_instance_service: IdentityInstanceServiceImpl<R, D>,
}

impl<R, D> OperatorApplication<R, D> {
    pub fn new(repository: Arc<R>, deployer: Arc<D>) -> Self {
        Self {
            identity_instance_service: IdentityInstanceServiceImpl::new(repository, deployer),
        }
    }
}

impl<R, D> IdentityInstanceService for OperatorApplication<R, D>
where
    R: IdentityInstanceRepository,
    D: IdentityInstanceDeployer,
{
    async fn reconcile(
        &self,
        instance: IdentityInstance,
    ) -> Result<ReconcileOutcome, OperatorError> {
        self.identity_instance_service.reconcile(instance).await
    }
}
