use aether_auth::Identity;
use aether_domain::platform::{
    EstatePage, EstateQuery, TenantPage, TenantQuery, ports::PlatformService,
    service::PlatformServiceImpl,
};
use aether_macros::transactional;

use crate::{AetherService, CoreError, infrastructure::role::permissions_in, policy::AetherPolicy};

impl PlatformService for AetherService {
    #[transactional(estate)]
    async fn list_estate_deployments(
        &self,
        identity: Identity,
        query: EstateQuery,
    ) -> Result<EstatePage, CoreError> {
        PlatformServiceImpl::new(estate_repository, AetherPolicy::new(permissions_in(&tx)))
            .list_estate_deployments(identity, query)
            .await
    }

    #[transactional(estate)]
    async fn list_tenants(
        &self,
        identity: Identity,
        query: TenantQuery,
    ) -> Result<TenantPage, CoreError> {
        PlatformServiceImpl::new(estate_repository, AetherPolicy::new(permissions_in(&tx)))
            .list_tenants(identity, query)
            .await
    }
}
