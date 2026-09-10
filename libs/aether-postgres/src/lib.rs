#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

// The attribute macros emit absolute `::aether_postgres::…` paths, which do
// not resolve from inside this crate without a self-alias. Repositories in
// this crate carry `#[repository]`, so the alias is load-bearing.
extern crate self as aether_postgres;

#[cfg_attr(coverage_nightly, coverage(off))]
pub mod action;

#[cfg_attr(coverage_nightly, coverage(off))]
pub mod catalog;

#[cfg_attr(coverage_nightly, coverage(off))]
pub mod deployments;

#[cfg_attr(coverage_nightly, coverage(off))]
pub mod dataplane;

#[cfg_attr(coverage_nightly, coverage(off))]
pub mod organisation;

#[cfg_attr(coverage_nightly, coverage(off))]
pub mod role;

#[cfg_attr(coverage_nightly, coverage(off))]
pub mod user;

/// Turns an sqlx failure into the domain's error.
///
/// It lives here because this is the only crate that knows both types: the
/// domain does not depend on sqlx, and `aether-persistence` does not depend on
/// the domain. `#[transactional]` hands this to `with_tx`.
pub fn map_sqlx_error(error: sqlx::Error) -> aether_domain::CoreError {
    aether_domain::CoreError::DatabaseError {
        message: error.to_string(),
    }
}

/// Maps a domain marker to the concrete repository a backend provides for it.
///
/// This exists so `#[transactional(deployment, action)]` can resolve
/// `deployment` to `PostgresDeploymentRepository` by type rather than by
/// parsing a name into an identifier and hoping it exists. A missing
/// registration is a compile error naming the domain, not a macro expanding
/// into an unresolved path.
///
/// Repositories register themselves with `#[repository(domain = ..., backend
/// = ...)]`; nothing here needs editing when one is added.
pub mod registry {
    use aether_persistence::SharedTx;

    /// Zero-sized markers, one per bounded context that owns a repository.
    /// They carry no data and exist only to key the `RepoFor` lookup.
    pub mod domain {
        pub struct Action;
        pub struct DataPlane;
        pub struct Deployment;
        pub struct Release;
        pub struct Organisation;
        pub struct Role;
        pub struct User;
    }

    pub mod backend {
        pub struct Postgres;
    }

    /// The backend in use. A second one would be added as another marker and
    /// selected here; the call sites in the application layer never name it.
    pub type Backend = backend::Postgres;

    pub trait RepoFor<D> {
        type Repo<'tx>;

        fn build<'tx>(tx: &SharedTx<'tx>) -> Self::Repo<'tx>;
    }
}

/// Asserts every domain marker has a repository registered against it.
///
/// `#[transactional(x)]` resolves `x` through this table, so a repository that
/// lost its `#[repository]` attribute fails here — naming the domain — instead
/// of at whichever use case happened to ask for it first.
///
/// This lives inside the crate rather than under `tests/`: an integration test
/// is a separate crate, and the `RepoFor` impls it would need to see are
/// governed by the orphan rule. That constraint is also why the registry sits
/// here beside the repositories rather than in `aether-persistence` next to
/// `SharedTx`.
#[cfg(test)]
mod registry_completeness {
    use crate::registry::{Backend, RepoFor, domain};

    fn assert_registered<D>()
    where
        Backend: RepoFor<D>,
    {
    }

    #[test]
    fn every_domain_has_a_postgres_repository() {
        assert_registered::<domain::Action>();
        assert_registered::<domain::DataPlane>();
        assert_registered::<domain::Deployment>();
        assert_registered::<domain::Organisation>();
        assert_registered::<domain::Release>();
        assert_registered::<domain::Role>();
        assert_registered::<domain::User>();
    }
}
