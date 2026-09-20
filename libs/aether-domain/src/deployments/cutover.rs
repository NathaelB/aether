//! Moving which deployment serves a hostname.
//!
//! A deployment's hostname is `slug(name)` under its organisation's own
//! domain (#282) -- nothing else decides it. So moving a customer's traffic
//! from one deployment to another, without an outage on the only name they
//! use, is a name swap: the deployment taking over gets the name the
//! customer's hostname is derived from, and the one giving it up keeps
//! running under the name it is left with.
//!
//! Symmetric on purpose. Cutting back is the same swap with the arguments
//! reversed, not a second operation to build and defend separately.

use crate::CoreError;
use crate::deployments::{Deployment, DeploymentId, DeploymentName};
use crate::organisation::OrganisationId;
use crate::user::UserId;

/// Two deployments trading hostnames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CutoverCommand {
    pub organisation_id: OrganisationId,
    /// Ends up serving the hostname `demote` currently does.
    pub promote: DeploymentId,
    /// Ends up on the hostname `promote` currently gives up.
    pub demote: DeploymentId,
    pub requested_by: UserId,
}

/// What a cutover actually writes, checked before anything is.
///
/// Three names, not two: a direct two-row swap can transiently give both
/// deployments the same name inside the same transaction, and the
/// repository's own constraint -- proving two live deployments never hold
/// one hostname -- is not deferrable. `parking_name` is written to
/// `promote` first, precisely because nothing real can ever collide with
/// it, and only then does `demote`'s old name move onto it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutoverPlan {
    pub promote_id: DeploymentId,
    pub promote_new_name: DeploymentName,
    pub demote_id: DeploymentId,
    pub demote_new_name: DeploymentName,
    pub parking_name: DeploymentName,
}

/// Plans a cutover, refusing one that cannot be carried out.
///
/// Takes the two `Deployment`s already loaded rather than ids, so the
/// caller decides how -- and whether -- to check the two are actually
/// related by a restore, which needs a repository this function does not
/// have.
pub fn plan_cutover(promote: &Deployment, demote: &Deployment) -> Result<CutoverPlan, CoreError> {
    if promote.id == demote.id {
        return Err(CoreError::CutoverRefused {
            reason: "a deployment cannot cut over from itself".to_string(),
        });
    }

    if promote.organisation_id != demote.organisation_id {
        return Err(CoreError::CutoverRefused {
            reason: "both deployments must belong to the same organisation".to_string(),
        });
    }

    if promote.deleted_at.is_some() || demote.deleted_at.is_some() {
        return Err(CoreError::CutoverRefused {
            reason: "a deleted deployment holds no hostname to trade".to_string(),
        });
    }

    Ok(CutoverPlan {
        promote_id: promote.id,
        promote_new_name: demote.name.clone(),
        demote_id: demote.id,
        demote_new_name: promote.name.clone(),
        // Never served: written and immediately replaced, inside the same
        // transaction, before either real name lands on the other row.
        parking_name: DeploymentName(format!("cutover-{}", uuid::Uuid::new_v4())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        dataplane::value_objects::{DataPlaneId, DeploymentResources},
        deployments::{
            DeploymentKind, DeploymentStatus, environment::Environment, network::NetworkAccess,
        },
        organisation::OrganisationId,
        user::UserId,
        version::Version,
    };
    use uuid::Uuid;

    fn deployment(id: u128, organisation: u128, name: &str) -> Deployment {
        let at = chrono::Utc::now();
        Deployment {
            id: DeploymentId(Uuid::from_u128(id)),
            organisation_id: OrganisationId(Uuid::from_u128(organisation)),
            dataplane_id: DataPlaneId(Uuid::from_u128(9)),
            name: DeploymentName(name.to_string()),
            kind: DeploymentKind::Keycloak,
            version: Version::new(26, 0, 1),
            status: DeploymentStatus::Successful,
            environment: Environment::Production,
            namespace: format!("production-{name}"),
            offer: None,
            restored_from: None,
            resources: DeploymentResources::DEFAULT,
            created_by: UserId(Uuid::from_u128(3)),
            created_at: at,
            updated_at: at,
            deployed_at: None,
            deleted_at: None,
            auto_upgrade: Default::default(),
            maintenance_window: None,
            network_access: NetworkAccess::Open,
            last_verified_restore_at: None,
            last_restore_drill_seconds: None,
            log_shipping_enabled: false,
        }
    }

    /// The point of the whole plan: each deployment ends up with what the
    /// other one had.
    #[test]
    fn each_deployment_ends_up_with_the_others_name() {
        let source = deployment(1, 1, "acme-prod");
        let recovery = deployment(2, 1, "acme-recovery");

        let plan = plan_cutover(&recovery, &source).expect("both are live, in the same org");

        assert_eq!(
            plan.promote_new_name,
            DeploymentName("acme-prod".to_string())
        );
        assert_eq!(
            plan.demote_new_name,
            DeploymentName("acme-recovery".to_string())
        );
    }

    /// The whole reason there are three names, not two: neither real name is
    /// ever written to both rows in the same moment, so the constraint that
    /// proves two live deployments never share a hostname never has anything
    /// to catch.
    #[test]
    fn the_parking_name_never_collides_with_either_real_name() {
        let source = deployment(1, 1, "acme-prod");
        let recovery = deployment(2, 1, "acme-recovery");

        let plan = plan_cutover(&recovery, &source).unwrap();

        assert_ne!(plan.parking_name, plan.promote_new_name);
        assert_ne!(plan.parking_name, plan.demote_new_name);
    }

    /// Calling this with the arguments reversed is not a second operation --
    /// it is what a cutback is. Proven here: swapping back undoes the swap
    /// exactly.
    #[test]
    fn cutting_back_is_the_same_operation_reversed() {
        let source = deployment(1, 1, "acme-prod");
        let recovery = deployment(2, 1, "acme-recovery");

        let cutover = plan_cutover(&recovery, &source).unwrap();

        let mut recovery_after = recovery.clone();
        recovery_after.name = cutover.promote_new_name.clone();
        let mut source_after = source.clone();
        source_after.name = cutover.demote_new_name.clone();

        let cutback = plan_cutover(&source_after, &recovery_after).unwrap();

        assert_eq!(cutback.promote_new_name, source.name);
        assert_eq!(cutback.demote_new_name, recovery.name);
    }

    #[test]
    fn a_deployment_cannot_cut_over_from_itself() {
        let solo = deployment(1, 1, "acme-prod");

        let refused = plan_cutover(&solo, &solo).unwrap_err();

        assert!(matches!(refused, CoreError::CutoverRefused { .. }));
    }

    /// A cutover across organisations would hand one customer's hostname to
    /// another's deployment.
    #[test]
    fn deployments_in_different_organisations_cannot_cut_over() {
        let mine = deployment(1, 1, "acme-prod");
        let theirs = deployment(2, 2, "globex-prod");

        let refused = plan_cutover(&mine, &theirs).unwrap_err();

        assert!(matches!(refused, CoreError::CutoverRefused { .. }));
    }

    #[test]
    fn a_deleted_deployment_cannot_take_part() {
        let source = deployment(1, 1, "acme-prod");
        let mut gone = deployment(2, 1, "acme-recovery");
        gone.deleted_at = Some(chrono::Utc::now());

        let refused = plan_cutover(&gone, &source).unwrap_err();
        assert!(matches!(refused, CoreError::CutoverRefused { .. }));

        let refused = plan_cutover(&source, &gone).unwrap_err();
        assert!(matches!(refused, CoreError::CutoverRefused { .. }));
    }
}
