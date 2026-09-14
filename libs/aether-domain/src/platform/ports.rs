use std::future::Future;

use aether_auth::Identity;

use crate::{
    CoreError,
    organisation::OrganisationId,
    platform::{
        EstatePage, EstateQuery, PlatformOperator, PlatformRight, PlatformRights, Tenant,
        TenantPage, TenantQuery,
    },
};

/// Whether an identity may do a thing to the installation, as opposed to
/// inside an organisation.
///
/// A port rather than a test on the identity, for the reason every customer
/// right is already one: a decision taken inline in a service has no single
/// place to change when the answer stops being a boolean.
///
/// Behind it, the rights an identity was granted, read on every request. That
/// is what makes a revocation take effect on the next one rather than on the
/// next token.
pub trait PlatformPolicy: Send + Sync {
    /// Refuses unless this identity was granted that right, naming it when it
    /// refuses.
    ///
    /// A refusal that said only "insufficient permissions" would send an
    /// operator looking for a role inside an organisation, which is a
    /// different system answering a different question.
    fn require(
        &self,
        identity: Identity,
        right: PlatformRight,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    /// Everything this identity holds.
    ///
    /// Asked when the answer is not a yes or a no: a grant is checked against
    /// what the grantor holds, which needs the set rather than one question
    /// about it.
    fn rights_of(
        &self,
        identity: Identity,
    ) -> impl Future<Output = Result<PlatformRights, CoreError>> + Send;
}

/// Reading the estate.
///
/// Reads only, and it stays that way. A repository that could also write here
/// would be a way to change a tenant's deployment without going through the
/// rules that own it.
pub trait EstateRepository: Send + Sync {
    fn list_deployments(
        &self,
        query: &EstateQuery,
    ) -> impl Future<Output = Result<EstatePage, CoreError>> + Send;

    fn list_tenants(
        &self,
        query: &TenantQuery,
    ) -> impl Future<Output = Result<TenantPage, CoreError>> + Send;

    /// One organisation, with what it holds.
    ///
    /// Asked rather than found in a page: the listing is paginated, so an
    /// organisation somebody followed a link to may not be on the page the
    /// screen happens to have.
    fn find_tenant(
        &self,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<Option<Tenant>, CoreError>> + Send;
}

/// Who operates this installation.
///
/// Read on every request that needs a platform right, which is what makes a
/// revocation take effect on the next one rather than on the next token.
pub trait OperatorRepository: Send + Sync {
    fn find(
        &self,
        subject: &str,
    ) -> impl Future<Output = Result<Option<PlatformOperator>, CoreError>> + Send;

    fn list(&self) -> impl Future<Output = Result<Vec<PlatformOperator>, CoreError>> + Send;

    /// Writes the rights, replacing whatever was there.
    ///
    /// Replacing rather than adding: a grant says what somebody holds, so
    /// narrowing an operator is the same call as widening one, and there is no
    /// pair of operations that can disagree about the result.
    fn grant(
        &self,
        operator: PlatformOperator,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn revoke(&self, subject: &str) -> impl Future<Output = Result<(), CoreError>> + Send;

    /// How many operators hold a given right.
    ///
    /// Asked before a revocation, to refuse the one that would leave the
    /// installation with nobody able to grant anything again.
    fn holders_of(
        &self,
        right: PlatformRight,
    ) -> impl Future<Output = Result<usize, CoreError>> + Send;
}

/// What the API layer calls for the platform's own screens.
pub trait PlatformService: Send + Sync {
    fn list_estate_deployments(
        &self,
        identity: Identity,
        query: EstateQuery,
    ) -> impl Future<Output = Result<EstatePage, CoreError>> + Send;

    /// Every organisation on the installation.
    ///
    /// Which tenants exist, what they are called and what they pay for is not
    /// a fact this platform tells anybody who holds a token. Somebody asking
    /// which organisations *they* belong to is a different question, answered
    /// by a path scoped to them.
    fn list_tenants(
        &self,
        identity: Identity,
        query: TenantQuery,
    ) -> impl Future<Output = Result<TenantPage, CoreError>> + Send;

    /// One organisation on the installation.
    fn get_tenant(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<Tenant, CoreError>> + Send;

    fn list_operators(
        &self,
        identity: Identity,
    ) -> impl Future<Output = Result<Vec<PlatformOperator>, CoreError>> + Send;

    /// What the caller holds.
    ///
    /// Needs no right of its own: asking what you may do is not a way to learn
    /// anything you may not. The console asks it to decide whether to draw the
    /// platform section at all, which it used to decide by decoding a realm
    /// role out of the token -- a rule that stopped being the one the API
    /// applies.
    fn my_platform_rights(
        &self,
        identity: Identity,
    ) -> impl Future<Output = Result<PlatformRights, CoreError>> + Send;

    /// Grants exactly `rights` to `subject`, replacing what they held.
    fn grant_operator(
        &self,
        identity: Identity,
        subject: String,
        rights: PlatformRights,
    ) -> impl Future<Output = Result<PlatformOperator, CoreError>> + Send;

    fn revoke_operator(
        &self,
        identity: Identity,
        subject: String,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;
}
