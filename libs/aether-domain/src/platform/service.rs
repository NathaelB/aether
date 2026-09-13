use aether_auth::Identity;

use crate::{
    CoreError,
    platform::{
        EstatePage, EstateQuery,
        ports::{EstateRepository, PlatformPolicy, PlatformService},
    },
};

pub struct PlatformServiceImpl<E, P>
where
    E: EstateRepository,
    P: PlatformPolicy,
{
    estate: E,
    policy: P,
}

impl<E, P> PlatformServiceImpl<E, P>
where
    E: EstateRepository,
    P: PlatformPolicy,
{
    pub fn new(estate: E, policy: P) -> Self {
        Self { estate, policy }
    }
}

impl<E, P> PlatformService for PlatformServiceImpl<E, P>
where
    E: EstateRepository,
    P: PlatformPolicy,
{
    async fn list_estate_deployments(
        &self,
        identity: Identity,
        query: EstateQuery,
    ) -> Result<EstatePage, CoreError> {
        // Before the read, not after. A refusal that first fetched the rows has
        // already done the thing it is refusing, and the rows here are every
        // tenant on the installation.
        self.policy.can_view_estate(identity).await?;

        self.estate.list_deployments(&query).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use super::*;
    use crate::platform::EstatePage;

    /// Records whether it was reached, which is the whole assertion below.
    #[derive(Clone, Default)]
    struct SpyEstate {
        read: Arc<AtomicBool>,
    }

    impl EstateRepository for SpyEstate {
        async fn list_deployments(&self, _query: &EstateQuery) -> Result<EstatePage, CoreError> {
            self.read.store(true, Ordering::SeqCst);

            Ok(EstatePage {
                deployments: Vec::new(),
                next_cursor: None,
            })
        }
    }

    struct Answer(bool);

    impl PlatformPolicy for Answer {
        async fn can_view_estate(&self, _identity: Identity) -> Result<(), CoreError> {
            if self.0 {
                return Ok(());
            }

            Err(CoreError::PermissionDenied {
                reason: "running the installation is a separate right".to_string(),
            })
        }
    }

    fn caller() -> Identity {
        Identity::Client(aether_auth::Client {
            id: "id".to_string(),
            client_id: "somebody".to_string(),
            roles: vec![],
            scopes: vec![],
        })
    }

    /// The refusal happens before the read. Otherwise every tenant's
    /// deployments have already been fetched out of the database by the time
    /// the caller is told they may not see them.
    #[tokio::test]
    async fn a_refusal_never_reaches_the_estate() {
        let estate = SpyEstate::default();
        let read = estate.read.clone();

        let refused = PlatformServiceImpl::new(estate, Answer(false))
            .list_estate_deployments(caller(), EstateQuery::new(None, None).unwrap())
            .await;

        assert!(matches!(refused, Err(CoreError::PermissionDenied { .. })));
        assert!(
            !read.load(Ordering::SeqCst),
            "the estate was read for a caller who may not see it"
        );
    }

    #[tokio::test]
    async fn an_operator_reads_the_estate() {
        let estate = SpyEstate::default();
        let read = estate.read.clone();

        let page = PlatformServiceImpl::new(estate, Answer(true))
            .list_estate_deployments(caller(), EstateQuery::new(None, None).unwrap())
            .await;

        assert!(page.is_ok());
        assert!(read.load(Ordering::SeqCst));
    }
}
