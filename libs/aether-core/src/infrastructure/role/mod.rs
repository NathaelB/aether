mod permission_provider;

pub use permission_provider::RolePermissionProvider;

use aether_persistence::SharedTx;
use aether_postgres::{organisation::PostgresOrganisationRepository, user::PostgresUserRepository};

/// The permission provider every policy is built with.
///
/// Composed in one place because the answer to "what may this caller do here"
/// must not depend on which use case is asking. Built on the surrounding
/// transaction, so a check cannot miss a role, a membership or an ownership
/// the same transaction has just written.
pub fn permissions_in<'tx>(
    tx: &SharedTx<'tx>,
) -> RolePermissionProvider<PostgresOrganisationRepository<'tx>, PostgresUserRepository<'tx>> {
    RolePermissionProvider::new(
        PostgresOrganisationRepository::new(tx),
        PostgresUserRepository::new(tx),
    )
}
