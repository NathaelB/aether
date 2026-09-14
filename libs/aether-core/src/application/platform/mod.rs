use aether_auth::Identity;
use aether_domain::{
    organisation::OrganisationId,
    platform::{
        EstatePage, EstateQuery, PlatformOperator, PlatformRights, Tenant, TenantPage, TenantQuery,
        ports::PlatformService, service::PlatformServiceImpl,
    },
};
use aether_macros::transactional;

use crate::{AetherService, CoreError, policy::PlatformRightsPolicy};

/// Whether the installation had to name its first operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstOperator {
    /// They already held rights, and nothing was changed.
    AlreadyGranted,
    Granted,
}

impl AetherService {
    /// Grants a subject every platform right, if it holds none.
    ///
    /// Not part of [`PlatformService`], and deliberately not authorised: there
    /// is no identity to authorise it against. Platform rights are granted by
    /// somebody who already holds one, which is a chain with no first link --
    /// a fresh database has nobody to grant the first. The installation says
    /// who that is, and this is where it lands.
    ///
    /// It never narrows and never revokes. An operator the installation named
    /// who was later given fewer rights keeps them: the configuration is a way
    /// back in when nobody can get in, not a standing instruction about what
    /// somebody holds.
    #[transactional(platform_operator)]
    pub async fn ensure_first_operator(&self, subject: &str) -> Result<FirstOperator, CoreError> {
        use aether_domain::platform::ports::OperatorRepository;

        if platform_operator_repository.find(subject).await?.is_some() {
            return Ok(FirstOperator::AlreadyGranted);
        }

        platform_operator_repository
            .grant(PlatformOperator {
                subject: subject.to_string(),
                rights: PlatformRights::everything(),
                // Nobody granted the first operator. Recording a subject here
                // would be inventing one.
                granted_by: None,
                granted_at: chrono::Utc::now(),
            })
            .await?;

        Ok(FirstOperator::Granted)
    }

    /// How many identities hold a platform right.
    ///
    /// Asked at startup to say, on the first line of the logs, that an
    /// installation nobody can operate is about to refuse every platform
    /// screen -- which is otherwise found in the database, hours later.
    #[transactional(platform_operator)]
    pub async fn count_operators(&self) -> Result<usize, CoreError> {
        use aether_domain::platform::ports::OperatorRepository;

        Ok(platform_operator_repository.list().await?.len())
    }
}

impl PlatformService for AetherService {
    #[transactional(estate, platform_operator)]
    async fn list_estate_deployments(
        &self,
        identity: Identity,
        query: EstateQuery,
    ) -> Result<EstatePage, CoreError> {
        PlatformServiceImpl::new(
            estate_repository,
            aether_postgres::platform::PostgresOperatorRepository::new(&tx),
            PlatformRightsPolicy::new(platform_operator_repository),
        )
        .list_estate_deployments(identity, query)
        .await
    }

    #[transactional(estate, platform_operator)]
    async fn list_tenants(
        &self,
        identity: Identity,
        query: TenantQuery,
    ) -> Result<TenantPage, CoreError> {
        PlatformServiceImpl::new(
            estate_repository,
            aether_postgres::platform::PostgresOperatorRepository::new(&tx),
            PlatformRightsPolicy::new(platform_operator_repository),
        )
        .list_tenants(identity, query)
        .await
    }

    #[transactional(estate, platform_operator)]
    async fn get_tenant(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> Result<Tenant, CoreError> {
        PlatformServiceImpl::new(
            estate_repository,
            aether_postgres::platform::PostgresOperatorRepository::new(&tx),
            PlatformRightsPolicy::new(platform_operator_repository),
        )
        .get_tenant(identity, organisation_id)
        .await
    }

    #[transactional(estate, platform_operator)]
    async fn list_operators(&self, identity: Identity) -> Result<Vec<PlatformOperator>, CoreError> {
        PlatformServiceImpl::new(
            estate_repository,
            aether_postgres::platform::PostgresOperatorRepository::new(&tx),
            PlatformRightsPolicy::new(platform_operator_repository),
        )
        .list_operators(identity)
        .await
    }

    #[transactional(estate, platform_operator)]
    async fn my_platform_rights(&self, identity: Identity) -> Result<PlatformRights, CoreError> {
        PlatformServiceImpl::new(
            estate_repository,
            aether_postgres::platform::PostgresOperatorRepository::new(&tx),
            PlatformRightsPolicy::new(platform_operator_repository),
        )
        .my_platform_rights(identity)
        .await
    }

    /// One transaction for the check and the write. The rule that an
    /// installation keeps somebody able to manage it counts the rows it is
    /// about to change; two grants landing together would each count the other
    /// as the survivor and both step down.
    #[transactional(estate, platform_operator)]
    async fn grant_operator(
        &self,
        identity: Identity,
        subject: String,
        rights: PlatformRights,
    ) -> Result<PlatformOperator, CoreError> {
        PlatformServiceImpl::new(
            estate_repository,
            aether_postgres::platform::PostgresOperatorRepository::new(&tx),
            PlatformRightsPolicy::new(platform_operator_repository),
        )
        .grant_operator(identity, subject, rights)
        .await
    }

    #[transactional(estate, platform_operator)]
    async fn revoke_operator(&self, identity: Identity, subject: String) -> Result<(), CoreError> {
        PlatformServiceImpl::new(
            estate_repository,
            aether_postgres::platform::PostgresOperatorRepository::new(&tx),
            PlatformRightsPolicy::new(platform_operator_repository),
        )
        .revoke_operator(identity, subject)
        .await
    }
}
