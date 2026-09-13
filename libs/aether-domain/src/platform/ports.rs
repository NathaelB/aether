use std::future::Future;

use aether_auth::Identity;

use crate::{
    CoreError,
    platform::{EstatePage, EstateQuery},
};

/// Whether an identity may do a thing to the installation, as opposed to
/// inside an organisation.
///
/// A port rather than a test on the identity, for the reason every customer
/// right is already one: an inline `is_operator()` is a decision taken in a
/// service, and there is no single place to change it when the answer stops
/// being a boolean. See #241 -- this trait is the seam that issue replaces
/// behind.
pub trait PlatformPolicy: Send + Sync {
    /// Reading what the installation runs: its data planes, its deployments,
    /// its organisations, its catalogue.
    ///
    /// A privilege in its own right. Which tenants exist and what they run is
    /// not a fact this platform tells anybody who asks.
    fn can_view_estate(
        &self,
        identity: Identity,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;
}

/// Reading the estate.
///
/// One method and it stays narrow. A repository that could also write here
/// would be a way to change a tenant's deployment without going through the
/// rules that own it.
pub trait EstateRepository: Send + Sync {
    fn list_deployments(
        &self,
        query: &EstateQuery,
    ) -> impl Future<Output = Result<EstatePage, CoreError>> + Send;
}

/// What the API layer calls for the platform's own screens.
pub trait PlatformService: Send + Sync {
    fn list_estate_deployments(
        &self,
        identity: Identity,
        query: EstateQuery,
    ) -> impl Future<Output = Result<EstatePage, CoreError>> + Send;
}
