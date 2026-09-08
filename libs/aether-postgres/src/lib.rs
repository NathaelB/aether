#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

// The attribute macros emit absolute `::aether_postgres::…` paths, which do
// not resolve from inside this crate without a self-alias. Repositories in
// this crate carry `#[repository]`, so the alias is load-bearing.
extern crate self as aether_postgres;

#[cfg_attr(coverage_nightly, coverage(off))]
pub mod action;

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

/// Proves the two attribute macros expand into code that compiles and runs.
///
/// The unit tests in `aether-macros` only cover `pascal_case`; a proc macro
/// that is never expanded is not tested. This exercises the whole seam with a
/// fake repository, so it stays true whatever the real repositories do.
///
/// It lives inside the crate rather than under `tests/`, and that is not a
/// stylistic choice: an integration test is a separate crate, so its
/// `RepoFor` impls would violate the orphan rule. The same constraint is why
/// the registry lives here at all, beside the repositories, rather than in
/// `aether-persistence` next to `SharedTx`.
#[cfg(test)]
mod macro_expansion {
    use aether_macros::{repository, transactional};
    use aether_persistence::SharedTx;
    use sqlx::PgPool;
    use sqlx::postgres::PgPoolOptions;

    use crate::registry::RepoFor;

    #[derive(Debug)]
    struct TestError(String);

    impl From<sqlx::Error> for TestError {
        fn from(error: sqlx::Error) -> Self {
            Self(error.to_string())
        }
    }

    #[repository(domain = Deployment, backend = Postgres)]
    pub struct FakeDeploymentRepository<'tx> {
        _tx: SharedTx<'tx>,
    }

    impl<'tx> FakeDeploymentRepository<'tx> {
        fn new(tx: &SharedTx<'tx>) -> Self {
            Self { _tx: tx.clone() }
        }
    }

    #[repository(domain = Action, backend = Postgres)]
    pub struct FakeActionRepository<'tx> {
        _tx: SharedTx<'tx>,
    }

    impl<'tx> FakeActionRepository<'tx> {
        fn new(tx: &SharedTx<'tx>) -> Self {
            Self { _tx: tx.clone() }
        }
    }

    struct FakeUseCase {
        pool: PgPool,
    }

    impl FakeUseCase {
        fn pool(&self) -> &PgPool {
            &self.pool
        }

        /// `deployment_repository` and `action_repository` are introduced by
        /// the attribute, not by this body. That they resolve at all is what
        /// proves the registry lookup works: nothing here names
        /// `FakeDeploymentRepository`.
        #[transactional(deployment, action)]
        async fn two_repositories_in_one_transaction(&self) -> Result<usize, TestError> {
            let _ = &deployment_repository;
            let _ = &action_repository;
            Ok(2)
        }
    }

    /// The body cannot run without a database, so this asserts the part that
    /// needs none: the expansion typechecks, both domains resolve through
    /// `RepoFor`, and a transaction that never opens surfaces as the caller's
    /// own error type instead of panicking.
    #[tokio::test]
    async fn transactional_expands_and_resolves_every_listed_domain() {
        let use_case = FakeUseCase {
            pool: PgPoolOptions::new()
                .acquire_timeout(std::time::Duration::from_millis(50))
                .connect_lazy("postgres://user:pass@127.0.0.1:1/db")
                .expect("valid database url"),
        };

        let result = use_case.two_repositories_in_one_transaction().await;

        let Err(TestError(message)) = result else {
            panic!("an unreachable database must not commit");
        };
        assert!(
            !message.is_empty(),
            "the sqlx failure must survive the conversion"
        );
    }

    /// Asserted separately from the use case, so a failure points at the
    /// registration rather than at the transaction.
    #[test]
    fn repository_registers_the_backend_implementation() {
        fn assert_registered<D>()
        where
            crate::registry::Backend: RepoFor<D>,
        {
        }

        assert_registered::<crate::registry::domain::Deployment>();
        assert_registered::<crate::registry::domain::Action>();
    }
}
