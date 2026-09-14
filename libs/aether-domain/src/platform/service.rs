use aether_auth::Identity;
use chrono::Utc;

use crate::{
    CoreError,
    platform::{
        EstatePage, EstateQuery, PlatformOperator, PlatformRight, PlatformRights, TenantPage,
        TenantQuery,
        ports::{EstateRepository, OperatorRepository, PlatformPolicy, PlatformService},
    },
};

pub struct PlatformServiceImpl<E, O, P>
where
    E: EstateRepository,
    O: OperatorRepository,
    P: PlatformPolicy,
{
    estate: E,
    operators: O,
    policy: P,
}

impl<E, O, P> PlatformServiceImpl<E, O, P>
where
    E: EstateRepository,
    O: OperatorRepository,
    P: PlatformPolicy,
{
    pub fn new(estate: E, operators: O, policy: P) -> Self {
        Self {
            estate,
            operators,
            policy,
        }
    }
}

impl<E, O, P> PlatformService for PlatformServiceImpl<E, O, P>
where
    E: EstateRepository,
    O: OperatorRepository,
    P: PlatformPolicy,
{
    async fn list_estate_deployments(
        &self,
        identity: Identity,
        query: EstateQuery,
    ) -> Result<EstatePage, CoreError> {
        // Before the read, not after. A refusal that first fetched the rows
        // has already done the thing it is refusing, and the rows here are
        // every tenant on the installation.
        self.policy
            .require(identity, PlatformRight::ViewEstate)
            .await?;

        self.estate.list_deployments(&query).await
    }

    async fn list_tenants(
        &self,
        identity: Identity,
        query: TenantQuery,
    ) -> Result<TenantPage, CoreError> {
        self.policy
            .require(identity, PlatformRight::ViewEstate)
            .await?;

        self.estate.list_tenants(&query).await
    }

    async fn list_operators(&self, identity: Identity) -> Result<Vec<PlatformOperator>, CoreError> {
        // Reading who operates the installation is part of seeing it. Somebody
        // who may look at every tenant learning who else may is not the leak.
        self.policy
            .require(identity, PlatformRight::ViewEstate)
            .await?;

        self.operators.list().await
    }

    async fn grant_operator(
        &self,
        identity: Identity,
        subject: String,
        rights: PlatformRights,
    ) -> Result<PlatformOperator, CoreError> {
        self.policy
            .require(identity.clone(), PlatformRight::ManageOperators)
            .await?;

        // Nobody hands out what they were not given. Without this, an operator
        // granted only `manage_operators` writes themselves a second account
        // holding everything, and the split into rights buys nothing.
        let held = self.policy.rights_of(identity.clone()).await?;
        if !held.covers(&rights) {
            return Err(CoreError::CannotGrantWhatYouDoNotHold {
                rights: held
                    .missing_from(&rights)
                    .iter()
                    .map(PlatformRight::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
            });
        }

        // An empty grant is a revocation written the long way, and a row
        // holding nothing would answer "yes, they are an operator" to every
        // screen that lists them.
        if rights.is_empty() {
            return self
                .revoke_operator(identity, subject.clone())
                .await
                .map(|()| PlatformOperator {
                    subject,
                    rights: PlatformRights::default(),
                    granted_by: None,
                    granted_at: Utc::now(),
                });
        }

        self.refuse_if_last_manager(&subject, &rights).await?;

        let operator = PlatformOperator {
            subject,
            rights,
            granted_by: Some(identity.id().to_string()),
            granted_at: Utc::now(),
        };
        self.operators.grant(operator.clone()).await?;

        Ok(operator)
    }

    async fn revoke_operator(&self, identity: Identity, subject: String) -> Result<(), CoreError> {
        self.policy
            .require(identity, PlatformRight::ManageOperators)
            .await?;

        self.refuse_if_last_manager(&subject, &PlatformRights::default())
            .await?;

        self.operators.revoke(&subject).await
    }
}

impl<E, O, P> PlatformServiceImpl<E, O, P>
where
    E: EstateRepository,
    O: OperatorRepository,
    P: PlatformPolicy,
{
    /// Refuses a change that would leave nobody able to grant rights again.
    ///
    /// The same shape as an organisation refusing to remove its owner, and for
    /// the same reason: there is nobody above an installation's administrators
    /// to restore them, so the recovery would be editing the database by hand
    /// on a platform somebody is currently trying to operate.
    async fn refuse_if_last_manager(
        &self,
        subject: &str,
        after: &PlatformRights,
    ) -> Result<(), CoreError> {
        if after.holds(PlatformRight::ManageOperators) {
            return Ok(());
        }

        let held_by_subject = self
            .operators
            .find(subject)
            .await?
            .is_some_and(|operator| operator.rights.holds(PlatformRight::ManageOperators));

        if !held_by_subject {
            return Ok(());
        }

        if self
            .operators
            .holders_of(PlatformRight::ManageOperators)
            .await?
            <= 1
        {
            return Err(CoreError::LastOperatorCannotBeRemoved);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use tokio::sync::Mutex;

    use super::*;
    use crate::platform::{EstatePage, TenantPage};

    /// Records whether it was reached, which is the whole assertion in the
    /// refusal tests below.
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

        async fn list_tenants(&self, _query: &TenantQuery) -> Result<TenantPage, CoreError> {
            self.read.store(true, Ordering::SeqCst);

            Ok(TenantPage {
                tenants: Vec::new(),
                next_cursor: None,
            })
        }
    }

    #[derive(Clone, Default)]
    struct InMemoryOperators {
        rows: Arc<Mutex<Vec<PlatformOperator>>>,
    }

    impl InMemoryOperators {
        async fn holding(rights: PlatformRights) -> Self {
            let store = Self::default();
            store.rows.lock().await.push(PlatformOperator {
                subject: "them".to_string(),
                rights,
                granted_by: None,
                granted_at: Utc::now(),
            });

            store
        }
    }

    impl OperatorRepository for InMemoryOperators {
        async fn find(&self, subject: &str) -> Result<Option<PlatformOperator>, CoreError> {
            Ok(self
                .rows
                .lock()
                .await
                .iter()
                .find(|operator| operator.subject == subject)
                .cloned())
        }

        async fn list(&self) -> Result<Vec<PlatformOperator>, CoreError> {
            Ok(self.rows.lock().await.clone())
        }

        async fn grant(&self, operator: PlatformOperator) -> Result<(), CoreError> {
            let mut rows = self.rows.lock().await;
            rows.retain(|held| held.subject != operator.subject);
            rows.push(operator);

            Ok(())
        }

        async fn revoke(&self, subject: &str) -> Result<(), CoreError> {
            self.rows
                .lock()
                .await
                .retain(|held| held.subject != subject);

            Ok(())
        }

        async fn holders_of(&self, right: PlatformRight) -> Result<usize, CoreError> {
            Ok(self
                .rows
                .lock()
                .await
                .iter()
                .filter(|operator| operator.rights.holds(right))
                .count())
        }
    }

    /// Answers from a fixed set, which is what the real one does after reading
    /// the row.
    struct Holding(PlatformRights);

    impl PlatformPolicy for Holding {
        async fn require(
            &self,
            _identity: Identity,
            right: PlatformRight,
        ) -> Result<(), CoreError> {
            if self.0.holds(right) {
                return Ok(());
            }

            Err(CoreError::MissingPlatformRight {
                right: right.to_string(),
            })
        }

        async fn rights_of(&self, _identity: Identity) -> Result<PlatformRights, CoreError> {
            Ok(self.0.clone())
        }
    }

    fn caller() -> Identity {
        Identity::Client(aether_auth::Client {
            id: "me".to_string(),
            client_id: "somebody".to_string(),
            roles: vec![],
            scopes: vec![],
        })
    }

    fn service(
        estate: SpyEstate,
        operators: InMemoryOperators,
        holding: PlatformRights,
    ) -> PlatformServiceImpl<SpyEstate, InMemoryOperators, Holding> {
        PlatformServiceImpl::new(estate, operators, Holding(holding))
    }

    /// The refusal happens before the read. Otherwise every tenant's
    /// deployments have been fetched out of the database by the time the
    /// caller is told they may not see them.
    #[tokio::test]
    async fn a_refusal_never_reaches_the_estate() {
        let estate = SpyEstate::default();
        let read = estate.read.clone();

        let refused = service(
            estate,
            InMemoryOperators::default(),
            PlatformRights::default(),
        )
        .list_estate_deployments(caller(), EstateQuery::new(None, None).unwrap())
        .await;

        assert!(matches!(
            refused,
            Err(CoreError::MissingPlatformRight { .. })
        ));
        assert!(
            !read.load(Ordering::SeqCst),
            "the estate was read for a caller who may not see it"
        );
    }

    #[tokio::test]
    async fn a_refusal_never_reaches_the_tenants_either() {
        let estate = SpyEstate::default();
        let read = estate.read.clone();

        let refused = service(
            estate,
            InMemoryOperators::default(),
            PlatformRights::default(),
        )
        .list_tenants(caller(), TenantQuery::new(None, None).unwrap())
        .await;

        assert!(matches!(
            refused,
            Err(CoreError::MissingPlatformRight { .. })
        ));
        assert!(!read.load(Ordering::SeqCst));
    }

    /// The split earns its keep here: reading the estate is not permission to
    /// change who operates it.
    #[tokio::test]
    async fn seeing_the_estate_is_not_permission_to_grant_it() {
        let refused = service(
            SpyEstate::default(),
            InMemoryOperators::default(),
            PlatformRights::of([PlatformRight::ViewEstate]),
        )
        .grant_operator(
            caller(),
            "them".to_string(),
            PlatformRights::of([PlatformRight::ViewEstate]),
        )
        .await
        .expect_err("a reader granted rights");

        let CoreError::MissingPlatformRight { right } = refused else {
            panic!("the refusal did not name a right");
        };
        assert_eq!(right, "manage_operators");
    }

    /// Without this, an operator granted only `manage_operators` writes
    /// themselves a second identity holding everything, and the split buys
    /// nothing at all.
    #[tokio::test]
    async fn nobody_hands_out_what_they_were_not_given() {
        let refused = service(
            SpyEstate::default(),
            InMemoryOperators::default(),
            PlatformRights::of([PlatformRight::ManageOperators, PlatformRight::ViewEstate]),
        )
        .grant_operator(
            caller(),
            "them".to_string(),
            PlatformRights::of([PlatformRight::ViewEstate, PlatformRight::ActOnTenant]),
        )
        .await
        .expect_err("a right nobody held was granted");

        let CoreError::CannotGrantWhatYouDoNotHold { rights } = refused else {
            panic!("the refusal was not about holding: {refused}");
        };
        assert_eq!(rights, "act_on_tenant", "the refusal names what is missing");
    }

    #[tokio::test]
    async fn an_operator_grants_what_they_hold() {
        let operators = InMemoryOperators::default();

        let granted = service(
            SpyEstate::default(),
            operators.clone(),
            PlatformRights::everything(),
        )
        .grant_operator(
            caller(),
            "them".to_string(),
            PlatformRights::of([PlatformRight::ViewEstate]),
        )
        .await
        .expect("a grant of what the caller holds");

        assert_eq!(granted.granted_by.as_deref(), Some("me"));
        assert_eq!(
            operators.find("them").await.unwrap().map(|o| o.rights),
            Some(PlatformRights::of([PlatformRight::ViewEstate]))
        );
    }

    /// An installation whose last administrator revoked themselves is one
    /// nobody can grant anything on again, and the recovery is editing the
    /// database by hand while somebody is trying to operate the platform.
    #[tokio::test]
    async fn the_last_administrator_cannot_revoke_themselves() {
        let operators =
            InMemoryOperators::holding(PlatformRights::of([PlatformRight::ManageOperators])).await;

        let refused = service(
            SpyEstate::default(),
            operators.clone(),
            PlatformRights::everything(),
        )
        .revoke_operator(caller(), "them".to_string())
        .await
        .expect_err("the last administrator was revoked");

        assert!(matches!(refused, CoreError::LastOperatorCannotBeRemoved));
        assert!(operators.find("them").await.unwrap().is_some());
    }

    /// Narrowing is the same refusal as revoking: an administrator reduced to
    /// a reader is an administrator gone.
    #[tokio::test]
    async fn the_last_administrator_cannot_be_narrowed_into_a_reader() {
        let operators =
            InMemoryOperators::holding(PlatformRights::of([PlatformRight::ManageOperators])).await;

        let refused = service(
            SpyEstate::default(),
            operators,
            PlatformRights::everything(),
        )
        .grant_operator(
            caller(),
            "them".to_string(),
            PlatformRights::of([PlatformRight::ViewEstate]),
        )
        .await
        .expect_err("the last administrator was narrowed away");

        assert!(matches!(refused, CoreError::LastOperatorCannotBeRemoved));
    }

    /// With a second administrator in place the same call goes through, which
    /// is what says the rule above is about the last one rather than about
    /// administrators in general.
    #[tokio::test]
    async fn one_of_two_administrators_may_step_down() {
        let operators =
            InMemoryOperators::holding(PlatformRights::of([PlatformRight::ManageOperators])).await;
        operators
            .grant(PlatformOperator {
                subject: "somebody-else".to_string(),
                rights: PlatformRights::of([PlatformRight::ManageOperators]),
                granted_by: None,
                granted_at: Utc::now(),
            })
            .await
            .unwrap();

        let stepped_down = service(
            SpyEstate::default(),
            operators.clone(),
            PlatformRights::everything(),
        )
        .revoke_operator(caller(), "them".to_string())
        .await;

        assert!(stepped_down.is_ok(), "{stepped_down:?}");
        assert!(operators.find("them").await.unwrap().is_none());
    }
}
