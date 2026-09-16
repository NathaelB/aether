//! Moving an organisation between plans, which is what decides the offers it
//! may choose from.
//!
//! Its own service rather than a method on the one that reads the estate:
//! that repository is read-only on purpose, and widening it would make every
//! screen able to write a tenant. This one holds the two things a write needs
//! and the estate holds neither -- the organisation itself, and somewhere to
//! record who moved it.
//!
//! The catalogue is not restated here. Which offers a plan opens is already
//! stated by the offer, in one direction, so moving a tenant between plans is
//! the whole of "managing what it may buy".

use aether_auth::Identity;
use chrono::Utc;
use uuid::Uuid;

use crate::{
    CoreError,
    audit::{
        AuditAction, AuditChange, AuditEntry, AuditEntryId, AuditTarget, AuditTargetKind,
        ports::AuditRepository, service::audit_actor,
    },
    organisation::{OrganisationId, ports::OrganisationRepository, value_objects::Plan},
    platform::{
        PlatformRight, Tenant,
        ports::{EstateRepository, PlatformPolicy, TenantPlanService},
    },
    user::ports::UserRepository,
};

pub struct TenantPlanServiceImpl<E, O, A, U, P>
where
    E: EstateRepository,
    O: OrganisationRepository,
    A: AuditRepository,
    U: UserRepository,
    P: PlatformPolicy,
{
    estate: E,
    organisations: O,
    audit: A,
    users: U,
    policy: P,
}

impl<E, O, A, U, P> TenantPlanServiceImpl<E, O, A, U, P>
where
    E: EstateRepository,
    O: OrganisationRepository,
    A: AuditRepository,
    U: UserRepository,
    P: PlatformPolicy,
{
    pub fn new(estate: E, organisations: O, audit: A, users: U, policy: P) -> Self {
        Self {
            estate,
            organisations,
            audit,
            users,
            policy,
        }
    }
}

impl<E, O, A, U, P> TenantPlanService for TenantPlanServiceImpl<E, O, A, U, P>
where
    E: EstateRepository,
    O: OrganisationRepository,
    A: AuditRepository,
    U: UserRepository,
    P: PlatformPolicy,
{
    /// Moves an organisation to a plan, and with it the offers it may choose.
    ///
    /// Hands back the tenant rather than the organisation, because the screen
    /// that asked is showing the counts beside the plan and would otherwise
    /// have to ask again for the half that did not change.
    async fn move_tenant_to_plan(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        plan: Plan,
    ) -> Result<Tenant, CoreError> {
        self.policy
            .require(identity.clone(), PlatformRight::ActOnTenant)
            .await?;

        let tenant = self.estate.find_tenant(organisation_id).await?.ok_or(
            CoreError::OrganisationNotFound {
                id: organisation_id.0,
            },
        )?;

        fits(
            &plan,
            "deployments",
            plan.max_instances(),
            tenant.deployments,
        )?;
        fits(&plan, "members", plan.max_users(), tenant.members)?;

        let was = tenant.organisation.plan;
        let mut organisation = tenant.organisation;

        // Nothing to write, and nothing worth recording: an operator who
        // opened the menu and chose what was already there did not act.
        if was == plan {
            return Ok(Tenant {
                organisation,
                ..tenant
            });
        }

        organisation.move_to_plan(plan)?;
        let organisation = self.organisations.update(organisation).await?;

        self.audit
            .append(AuditEntry::record(
                AuditEntryId(Uuid::new_v4()),
                organisation_id,
                audit_actor(&identity, &self.users).await?,
                AuditAction("organisation.plan.changed".to_string()),
                AuditTarget {
                    kind: AuditTargetKind::Organisation,
                    id: organisation_id.0,
                },
                Some(AuditChange::new(
                    serde_json::json!({ "plan": was.to_string() }),
                    serde_json::json!({ "plan": plan.to_string() }),
                )?),
                Utc::now(),
            ))
            .await?;

        Ok(Tenant {
            organisation,
            ..tenant
        })
    }
}

/// Refuses a plan that sits below what the organisation already holds.
///
/// Applying it anyway leaves a tenant above its own ceiling: it can create
/// nothing, and coming back under the line means deleting something somebody
/// is using. That is a conversation to have before the move, not after.
fn fits(plan: &Plan, what: &str, allowed: usize, in_use: usize) -> Result<(), CoreError> {
    if in_use > allowed {
        return Err(CoreError::PlanBelowWhatIsInUse {
            plan: plan.to_string(),
            what: what.to_string(),
            allowed,
            in_use,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use crate::{
        audit::{AuditEntry, ports::MockAuditRepository},
        organisation::{
            Organisation,
            ports::MockOrganisationRepository,
            value_objects::{
                OrganisationLimits, OrganisationName, OrganisationSlug, OrganisationStatus,
            },
        },
        platform::{EstatePage, EstateQuery, PlatformRights, TenantPage, TenantQuery},
        user::{User, ports::UserRepository},
    };
    use chrono::Utc;
    use uuid::Uuid;

    const ORGANISATION: Uuid = Uuid::from_u128(1);

    fn organisation(plan: Plan) -> Organisation {
        let now = Utc::now();

        Organisation {
            id: OrganisationId(ORGANISATION),
            name: OrganisationName::new("Acme").expect("a name"),
            slug: OrganisationSlug::new("acme").expect("a slug"),
            owner_id: crate::user::UserId(Uuid::from_u128(2)),
            status: OrganisationStatus::Active,
            plan,
            limits: OrganisationLimits::from_plan(&plan),
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    /// One tenant, with whatever it is said to hold.
    struct OneTenant(Tenant);

    impl EstateRepository for OneTenant {
        async fn list_deployments(&self, _query: &EstateQuery) -> Result<EstatePage, CoreError> {
            unreachable!("the plan service never lists the estate")
        }

        async fn list_tenants(&self, _query: &TenantQuery) -> Result<TenantPage, CoreError> {
            unreachable!("the plan service never lists tenants")
        }

        async fn find_tenant(
            &self,
            organisation_id: OrganisationId,
        ) -> Result<Option<Tenant>, CoreError> {
            Ok((organisation_id == self.0.organisation.id).then(|| self.0.clone()))
        }
    }

    struct NoUsers;

    impl UserRepository for NoUsers {
        async fn upsert_by_email(&self, _user: &User) -> Result<User, CoreError> {
            unreachable!("nothing here writes a user")
        }

        async fn find_by_sub(&self, _sub: &str) -> Result<Option<User>, CoreError> {
            unreachable!("the caller in these tests is a client, not a person")
        }

        async fn find_by_email(&self, _email: &str) -> Result<Option<User>, CoreError> {
            unreachable!("nothing here looks a user up by email")
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

    type Written = Arc<Mutex<Vec<Organisation>>>;
    type Recorded = Arc<Mutex<Vec<AuditEntry>>>;

    fn service(
        tenant: Tenant,
        holding: PlatformRights,
    ) -> (
        TenantPlanServiceImpl<
            OneTenant,
            MockOrganisationRepository,
            MockAuditRepository,
            NoUsers,
            Holding,
        >,
        Written,
        Recorded,
    ) {
        let written: Written = Arc::new(Mutex::new(Vec::new()));
        let recorded: Recorded = Arc::new(Mutex::new(Vec::new()));

        let mut organisations = MockOrganisationRepository::new();
        let kept = Arc::clone(&written);
        organisations
            .expect_update()
            .returning(move |organisation| {
                kept.lock().expect("the writes").push(organisation.clone());
                Box::pin(async move { Ok(organisation) })
            });

        let mut audit = MockAuditRepository::new();
        let heard = Arc::clone(&recorded);
        audit.expect_append().returning(move |entry| {
            heard.lock().expect("the entries").push(entry);
            Box::pin(async { Ok(()) })
        });

        (
            TenantPlanServiceImpl::new(
                OneTenant(tenant),
                organisations,
                audit,
                NoUsers,
                Holding(holding),
            ),
            written,
            recorded,
        )
    }

    fn tenant(plan: Plan, deployments: usize, members: usize) -> Tenant {
        Tenant {
            organisation: organisation(plan),
            deployments,
            members,
        }
    }

    fn acting() -> PlatformRights {
        PlatformRights::of([PlatformRight::ActOnTenant])
    }

    #[tokio::test]
    async fn an_operator_moves_an_organisation_to_another_plan() {
        let (service, written, recorded) = service(tenant(Plan::Free, 1, 1), acting());

        let moved = service
            .move_tenant_to_plan(caller(), OrganisationId(ORGANISATION), Plan::Business)
            .await
            .expect("moved");

        assert_eq!(moved.organisation.plan, Plan::Business);
        assert_eq!(written.lock().expect("the writes").len(), 1);
        assert_eq!(recorded.lock().expect("the entries").len(), 1);
    }

    /// The limits follow the plan rather than being a second thing to write.
    /// Two facts that can be written separately eventually disagree, and
    /// nothing then says which plan the organisation is on.
    #[tokio::test]
    async fn the_limits_follow_the_plan() {
        let (service, _written, _recorded) = service(tenant(Plan::Free, 1, 1), acting());

        let moved = service
            .move_tenant_to_plan(caller(), OrganisationId(ORGANISATION), Plan::Business)
            .await
            .expect("moved");

        assert_eq!(
            moved.organisation.limits,
            OrganisationLimits::from_plan(&Plan::Business)
        );
    }

    /// Applying it anyway leaves a tenant above its own ceiling: it can create
    /// nothing, and coming back under the line means deleting something
    /// somebody is using.
    #[tokio::test]
    async fn a_plan_below_what_is_already_running_is_refused_and_says_what_is_in_the_way() {
        let (service, written, _recorded) = service(tenant(Plan::Business, 8, 3), acting());

        let refused = service
            .move_tenant_to_plan(caller(), OrganisationId(ORGANISATION), Plan::Starter)
            .await
            .expect_err("a tenant was left over its own limit");

        let CoreError::PlanBelowWhatIsInUse {
            what,
            allowed,
            in_use,
            ..
        } = refused
        else {
            panic!("the refusal was not about what is in use: {refused}");
        };

        assert_eq!(what, "deployments");
        assert_eq!((allowed, in_use), (5, 8));
        assert!(
            written.lock().expect("the writes").is_empty(),
            "and nothing was written"
        );
    }

    #[tokio::test]
    async fn a_plan_with_room_for_fewer_members_than_there_are_is_refused_too() {
        let (service, _written, _recorded) = service(tenant(Plan::Business, 0, 40), acting());

        let refused = service
            .move_tenant_to_plan(caller(), OrganisationId(ORGANISATION), Plan::Starter)
            .await
            .expect_err("a tenant was left over its own limit");

        assert!(matches!(
            refused,
            CoreError::PlanBelowWhatIsInUse { ref what, .. } if what == "members"
        ));
    }

    /// Seeing every tenant is not permission to change what one of them pays.
    #[tokio::test]
    async fn seeing_the_estate_is_not_permission_to_move_a_tenant() {
        let (service, written, _recorded) = service(
            tenant(Plan::Free, 0, 1),
            PlatformRights::of([PlatformRight::ViewEstate]),
        );

        let refused = service
            .move_tenant_to_plan(caller(), OrganisationId(ORGANISATION), Plan::Enterprise)
            .await
            .expect_err("a reader moved a tenant");

        let CoreError::MissingPlatformRight { right } = refused else {
            panic!("the refusal did not name a right: {refused}");
        };
        assert_eq!(right, "act_on_tenant");
        assert!(written.lock().expect("the writes").is_empty());
    }

    #[tokio::test]
    async fn an_organisation_that_is_not_there_is_said_so() {
        let (service, _written, _recorded) = service(tenant(Plan::Free, 0, 1), acting());

        let refused = service
            .move_tenant_to_plan(caller(), OrganisationId(Uuid::from_u128(99)), Plan::Free)
            .await
            .expect_err("a tenant nobody has");

        assert!(matches!(refused, CoreError::OrganisationNotFound { .. }));
    }

    /// An operator who opened the menu and chose what was already there did
    /// not act, and a trail full of those is a trail nobody reads.
    #[tokio::test]
    async fn choosing_the_plan_it_is_already_on_writes_nothing() {
        let (service, written, recorded) = service(tenant(Plan::Business, 2, 2), acting());

        service
            .move_tenant_to_plan(caller(), OrganisationId(ORGANISATION), Plan::Business)
            .await
            .expect("accepted");

        assert!(written.lock().expect("the writes").is_empty());
        assert!(recorded.lock().expect("the entries").is_empty());
    }
}
