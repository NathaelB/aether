//! What the platform publishes, and to whom it is offered.
//!
//! The catalogue is the source of truth every other part of the upgrade
//! machinery reads: what exists, whether it may still be installed, and what
//! it is likely to break. It knows nothing about clusters or upgrades. It
//! answers one question, asked by everything else: may this version be
//! offered to this deployment, now.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    CoreError, deployments::DeploymentKind, organisation::value_objects::Plan, version::Version,
};

pub mod commands;
pub mod ports;
pub mod rollout;
pub mod service;

pub use rollout::{Rollout, RolloutCandidate, RolloutError, RolloutPercentage};

/// Identity of a release. Not a surrogate key: a version of a product is the
/// same release wherever it is referred to, and two rows for one of them is
/// the bug the unique index exists to prevent.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct ReleaseId {
    pub kind: DeploymentKind,
    pub version: Version,
}

impl ReleaseId {
    pub fn new(kind: DeploymentKind, version: Version) -> Self {
        Self { kind, version }
    }
}

impl std::fmt::Display for ReleaseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.kind, self.version)
    }
}

/// Where a release is in its life.
///
/// Ordered, and the order is the rule: a release only ever moves forward.
/// Coming back from `Withdrawn` would mean a version that was pulled can be
/// installed again, which is the one thing a withdrawal has to guarantee
/// against.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseStatus {
    /// Announced, not installable yet. Never offered to a customer.
    Upcoming,
    /// Installable, and offered as an upgrade to whoever the rollout covers.
    Available,
    /// Still runs, still supported, no longer the version to move to.
    Deprecated,
    /// Must not be installed or upgraded to. Does not stop what already runs
    /// it: breaking a running deployment is not what a withdrawal is for.
    Withdrawn,
}

impl ReleaseStatus {
    /// Whether this release can be installed or upgraded to.
    pub fn is_installable(self) -> bool {
        matches!(self, Self::Available | Self::Deprecated)
    }

    /// Whether a customer should ever see it.
    ///
    /// `Upcoming` is the platform's own planning. Showing it would advertise a
    /// date nobody committed to.
    pub fn is_visible_to_customers(self) -> bool {
        self != Self::Upcoming
    }
}

/// How much a release is expected to break.
///
/// Declared by whoever publishes it, never derived from the version number.
/// Semver states an intent; it does not describe what a third party actually
/// shipped, and a patch that changes a default is a config break whatever it
/// is numbered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BreakingRisk {
    /// Drop in.
    None,
    /// Runs, but configuration may need attention afterwards.
    Config,
    /// Will not run as configured today without a change.
    Breaking,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ReleaseNotes(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Release {
    pub id: ReleaseId,
    pub status: ReleaseStatus,
    pub risk: BreakingRisk,
    pub notes: ReleaseNotes,
    /// How much of the estate this release is offered to, on top of being
    /// `Available`. Separate from `status` on purpose -- see the module docs
    /// on [`ReleaseStatus`]: a withdrawn release is not installable whatever
    /// its rollout says, and a rollout answers a different question than
    /// status does even while the release is available.
    pub rollout: Rollout,
    /// Lowest operator/chart version a data plane must run to host a
    /// deployment on this release. `None` means any operator may install it.
    ///
    /// There is one operator per cluster shared by every tenant on it, so
    /// this is a fact about the infrastructure a release needs, not about who
    /// it is offered to -- which is why it lives beside `rollout` rather than
    /// inside it.
    pub minimum_operator_version: Option<Version>,
    /// Versions that must be passed through to reach this one.
    ///
    /// Declared by whoever publishes the release rather than worked out from
    /// the numbers: which migrations are mandatory is knowledge the product's
    /// authors have and the platform does not. Empty means it can be reached
    /// directly.
    pub steps_through: Vec<Version>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Release {
    /// A new release starts `Upcoming`, with no rollout restriction and no
    /// operator requirement.
    ///
    /// Publishing is a separate act from recording that a version exists. The
    /// two collapse into one only when nobody ever needs to prepare notes
    /// before customers can see them, and they always do.
    pub fn announce(
        id: ReleaseId,
        risk: BreakingRisk,
        notes: ReleaseNotes,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            status: ReleaseStatus::Upcoming,
            risk,
            notes,
            // Closed, not full: publishing a version puts it in the
            // catalogue, it does not hand it to the estate. Those are two
            // decisions and an operator should make the second one on
            // purpose.
            rollout: Rollout::closed(),
            minimum_operator_version: None,
            steps_through: Vec::new(),
            created_at: at,
            updated_at: at,
        }
    }

    /// Moves the release forward.
    ///
    /// Any forward step is allowed, not only the next one. Withdrawing an
    /// `Available` release without passing through `Deprecated` is exactly
    /// what happens when a version turns out to corrupt data, and forcing it
    /// through an intermediate state would mean an hour where it is still
    /// being installed.
    ///
    /// Backwards and sideways are refused. That is the whole rule.
    pub fn move_to(&mut self, status: ReleaseStatus, at: DateTime<Utc>) -> Result<(), CoreError> {
        if status <= self.status {
            return Err(CoreError::InvalidReleaseTransition {
                release: self.id.to_string(),
                from: format!("{:?}", self.status).to_lowercase(),
                to: format!("{status:?}").to_lowercase(),
            });
        }

        self.status = status;
        self.updated_at = at;
        Ok(())
    }

    /// Notes, risk, the steps a path must pass through and the operator
    /// requirement all stay editable for as long as the release exists: what a
    /// version breaks, and what it turns out to need, is often discovered
    /// after it ships, and a catalogue that cannot record that is worth less
    /// than no catalogue.
    pub fn revise(
        &mut self,
        risk: BreakingRisk,
        notes: ReleaseNotes,
        steps_through: Vec<Version>,
        minimum_operator_version: Option<Version>,
        at: DateTime<Utc>,
    ) {
        self.risk = risk;
        self.notes = notes;
        self.steps_through = steps_through;
        self.minimum_operator_version = minimum_operator_version;
        self.updated_at = at;
    }

    /// Sets the release's rollout: wholesale while nobody could have seen the
    /// release yet, widen-only from the moment they could have.
    ///
    /// `Upcoming` is never shown to a customer, so nothing has been offered
    /// under whatever the rollout said up to that point -- an operator
    /// preparing a release is sculpting a plan, not narrowing an offer
    /// already made. This is also the answer to the question the module docs
    /// on [`Rollout`] leave open, "what does 'already offered' mean": exactly
    /// "has this release ever been visible", which `ReleaseStatus` already
    /// tracks and this method reads rather than duplicating.
    ///
    /// Once visible, [`Rollout`]'s own widen-only rule applies and this
    /// simply carries its refusal outward.
    pub fn widen_rollout(
        &mut self,
        target: Rollout,
        at: DateTime<Utc>,
    ) -> Result<(), RolloutError> {
        if self.status.is_visible_to_customers() {
            self.rollout.widen_percentage(target.percentage())?;
            self.rollout
                .widen_plans(target.plans().map(<[Plan]>::to_vec))?;
            self.rollout
                .add_pilot_organisations(target.pilot_organisations().iter().copied());
        } else {
            self.rollout = target;
        }

        self.updated_at = at;
        Ok(())
    }

    pub fn version(&self) -> &Version {
        &self.id.version
    }

    pub fn kind(&self) -> &DeploymentKind {
        &self.id.kind
    }
}

/// A release and how much of the estate is on it.
///
/// A read model, not part of the aggregate: how many deployments run a version
/// is a fact about the estate at one moment, not about the release. Putting it
/// on `Release` would mean every write carried a number nobody wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ReleaseInUse {
    #[serde(flatten)]
    pub release: Release,
    /// Live deployments on this version, across every organisation. A
    /// deployment being torn down is not counted.
    pub deployments: u64,
}

/// How many deployments a candidate rollout would cover, before it is saved.
///
/// A read model, not part of the aggregate, for the same reason
/// [`ReleaseInUse`] is not: it is a fact about the estate at one moment, not
/// about the release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct RolloutCoverage {
    /// How many deployments of the product this rollout would offer the
    /// release to.
    pub covered: u64,
    /// How many deployments of the product exist at all, so "covered" can be
    /// read as a fraction rather than a bare number.
    pub total: u64,
}

/// A data plane whose operator/chart version is behind what a release needs.
///
/// A read model rather than a fact on `Release`: which data planes are behind
/// changes every time one reports a heartbeat, and the release itself neither
/// causes nor records that.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct HeldBackDataPlane {
    pub id: crate::dataplane::value_objects::DataPlaneId,
    /// `None` when the data plane has never reported one at all, which holds
    /// a release back exactly as hard as reporting one that is too old.
    pub operator_version: Option<Version>,
}

/// Why a release did not come back in [`ReleaseAvailability::eligible`],
/// named rather than left for the client to guess. An empty list and a list
/// of releases nobody may install look the same from the outside unless the
/// reason travels with the entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum IneligibilityReason {
    /// The catalogue holds it, but its status refuses installs -- `Upcoming`
    /// or `Withdrawn`.
    NotInstallable { status: ReleaseStatus },
    /// The data plane this deployment runs on has not reported an operator
    /// version at or above what the release requires.
    OperatorTooOld {
        minimum: Version,
        dataplane: Option<Version>,
    },
    /// Installable and the operator is new enough, but this deployment falls
    /// outside the rollout's percentage, plan targeting and pilot list.
    OutsideRollout,
}

/// A release as one specific deployment sees it: not just whether it may be
/// installed, but why, when it may not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ReleaseAvailability {
    #[serde(flatten)]
    pub release: Release,
    pub eligible: bool,
    /// Set exactly when `eligible` is `false`. A client that sees a release
    /// but not the reason has effectively seen nothing.
    pub reason: Option<IneligibilityReason>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .expect("a valid instant")
            .with_timezone(&Utc)
    }

    fn later() -> DateTime<Utc> {
        at() + chrono::Duration::days(1)
    }

    fn release(status: ReleaseStatus) -> Release {
        let mut release = Release::announce(
            ReleaseId::new(DeploymentKind::Ferriskey, Version::new(26, 0, 1)),
            BreakingRisk::None,
            ReleaseNotes("notes".to_string()),
            at(),
        );
        release.status = status;
        release
    }

    /// Recording that a version exists is not publishing it. Starting
    /// `Available` would put every draft in front of customers.
    #[test]
    fn a_release_starts_unpublished() {
        let release = release(ReleaseStatus::Upcoming);

        assert_eq!(release.status, ReleaseStatus::Upcoming);
        assert!(!release.status.is_installable());
        assert!(!release.status.is_visible_to_customers());
    }

    #[test]
    fn moves_forward_through_its_life() {
        let mut release = release(ReleaseStatus::Upcoming);

        for status in [
            ReleaseStatus::Available,
            ReleaseStatus::Deprecated,
            ReleaseStatus::Withdrawn,
        ] {
            release.move_to(status, later()).expect("a forward step");
            assert_eq!(release.status, status);
        }
    }

    /// A version found to corrupt data has to stop being installed now, not
    /// after a pass through deprecated during which it is still being
    /// installed.
    #[test]
    fn can_be_withdrawn_without_being_deprecated_first() {
        let mut release = release(ReleaseStatus::Available);

        release
            .move_to(ReleaseStatus::Withdrawn, later())
            .expect("an emergency withdrawal");

        assert_eq!(release.status, ReleaseStatus::Withdrawn);
    }

    /// The rule the whole status enum exists for. A withdrawn version that can
    /// come back is not withdrawn.
    #[test]
    fn never_moves_backwards() {
        let backwards = [
            (ReleaseStatus::Withdrawn, ReleaseStatus::Available),
            (ReleaseStatus::Withdrawn, ReleaseStatus::Deprecated),
            (ReleaseStatus::Deprecated, ReleaseStatus::Available),
            (ReleaseStatus::Available, ReleaseStatus::Upcoming),
        ];

        for (from, to) in backwards {
            let mut release = release(from);
            let error = release
                .move_to(to, later())
                .expect_err("a release never moves backwards");

            assert!(matches!(error, CoreError::InvalidReleaseTransition { .. }));
            assert_eq!(release.status, from, "the status did not move");
        }
    }

    #[test]
    fn a_status_is_not_a_step_from_itself() {
        for status in [
            ReleaseStatus::Upcoming,
            ReleaseStatus::Available,
            ReleaseStatus::Deprecated,
            ReleaseStatus::Withdrawn,
        ] {
            let mut release = release(status);

            assert!(
                release.move_to(status, later()).is_err(),
                "{status:?} is not a step from itself"
            );
        }
    }

    /// The refusal names the release and both ends of the step, because the
    /// person reading it is looking at a list of versions and needs to know
    /// which one refused.
    #[test]
    fn the_refusal_says_which_release_and_which_step() {
        let mut release = release(ReleaseStatus::Withdrawn);
        let error = release
            .move_to(ReleaseStatus::Available, later())
            .expect_err("backwards");
        let message = error.to_string();

        assert!(message.contains("26.0.1"), "{message}");
        assert!(message.contains("ferriskey"), "{message}");
        assert!(message.contains("withdrawn"), "{message}");
        assert!(message.contains("available"), "{message}");
    }

    /// What is installable and what a customer sees are two questions.
    /// Deprecated answers yes to the first: it still runs and can still be
    /// moved to, it is simply not where anyone should be heading.
    #[test]
    fn deprecated_is_still_installable_and_withdrawn_is_not() {
        assert!(ReleaseStatus::Available.is_installable());
        assert!(ReleaseStatus::Deprecated.is_installable());
        assert!(!ReleaseStatus::Withdrawn.is_installable());
        assert!(!ReleaseStatus::Upcoming.is_installable());
    }

    /// Upcoming is the platform's own planning. Showing it would advertise a
    /// date nobody committed to.
    #[test]
    fn only_upcoming_is_hidden_from_customers() {
        assert!(!ReleaseStatus::Upcoming.is_visible_to_customers());
        assert!(ReleaseStatus::Available.is_visible_to_customers());
        assert!(ReleaseStatus::Deprecated.is_visible_to_customers());
        assert!(ReleaseStatus::Withdrawn.is_visible_to_customers());
    }

    /// Risk is declared, never derived. A patch that changes a default breaks
    /// configuration whatever its number says.
    #[test]
    fn risk_is_whatever_was_declared() {
        let release = Release::announce(
            ReleaseId::new(DeploymentKind::Keycloak, Version::new(26, 0, 1)),
            BreakingRisk::Breaking,
            ReleaseNotes("changes a default".to_string()),
            at(),
        );

        assert_eq!(release.risk, BreakingRisk::Breaking);
    }

    /// What a version breaks is often found out after it ships.
    #[test]
    fn notes_and_risk_can_be_revised_after_publishing() {
        let mut release = release(ReleaseStatus::Available);

        release.revise(
            BreakingRisk::Breaking,
            ReleaseNotes("found to drop sessions on restart".to_string()),
            Vec::new(),
            None,
            later(),
        );

        assert_eq!(release.risk, BreakingRisk::Breaking);
        assert_eq!(release.updated_at, later());
        assert_eq!(
            release.status,
            ReleaseStatus::Available,
            "revising notes does not move the release"
        );
    }

    /// Two products can hold the same version number without being the same
    /// release.
    #[test]
    fn a_release_is_identified_by_its_product_and_its_version() {
        let ferriskey = ReleaseId::new(DeploymentKind::Ferriskey, Version::new(26, 0, 1));
        let keycloak = ReleaseId::new(DeploymentKind::Keycloak, Version::new(26, 0, 1));

        assert_ne!(ferriskey, keycloak);
        assert_eq!(
            ferriskey,
            ReleaseId::new(DeploymentKind::Ferriskey, Version::new(26, 0, 1))
        );
    }

    #[test]
    fn statuses_serialise_in_the_same_case_as_the_rest_of_the_api() {
        assert_eq!(
            serde_json::to_string(&ReleaseStatus::Withdrawn).expect("serialises"),
            "\"withdrawn\""
        );
        assert_eq!(
            serde_json::to_string(&BreakingRisk::Config).expect("serialises"),
            "\"config\""
        );
    }

    /// Publishing puts a version in the catalogue; it does not hand it to the
    /// estate. Those are two decisions, and a release that arrived offered to
    /// everyone would make the second one silently, at the moment somebody
    /// was only recording that the version exists.
    #[test]
    fn a_new_release_is_offered_to_nobody_until_it_is_widened() {
        let release = release(ReleaseStatus::Upcoming);

        assert_eq!(release.rollout, Rollout::closed());
        assert!(!release.rollout.is_global());
        assert_eq!(release.minimum_operator_version, None);
        assert!(release.steps_through.is_empty());
    }

    /// The one gesture that says a version is ready for everyone. It only
    /// goes this way: a version somebody has already taken cannot be
    /// un-offered, which is what withdrawing is for.
    #[test]
    fn widening_a_release_all_the_way_makes_it_global() {
        let mut release = release(ReleaseStatus::Available);

        release
            .widen_rollout(Rollout::full(), Utc::now())
            .expect("widening is allowed");

        assert!(release.rollout.is_global());
    }

    #[test]
    fn revising_a_release_can_declare_steps_and_an_operator_requirement() {
        let mut release = release(ReleaseStatus::Available);
        let steps = vec![Version::new(25, 5, 0)];
        let minimum_operator = Version::new(1, 4, 0);

        release.revise(
            release.risk,
            ReleaseNotes("notes".to_string()),
            steps.clone(),
            Some(minimum_operator.clone()),
            later(),
        );

        assert_eq!(release.steps_through, steps);
        assert_eq!(release.minimum_operator_version, Some(minimum_operator));
    }

    fn at_percent(percent: u8) -> Rollout {
        Rollout::new(RolloutPercentage::new(percent).unwrap(), None, Vec::new())
    }

    /// Nobody could have seen an `Upcoming` release, so preparing its rollout
    /// is sculpting a plan, not narrowing an offer -- any value is accepted,
    /// including one below the default `Rollout::full` every release starts
    /// with.
    #[test]
    fn an_upcoming_release_accepts_any_rollout_including_a_low_one() {
        let mut release = release(ReleaseStatus::Upcoming);

        release
            .widen_rollout(at_percent(30), later())
            .expect("nothing has been offered yet");

        assert_eq!(
            release.rollout.percentage(),
            RolloutPercentage::new(30).unwrap()
        );
        assert_eq!(release.updated_at, later());
    }

    /// The moment a release could have been seen, `Rollout`'s own widen-only
    /// rule takes over: a real widening still succeeds.
    #[test]
    fn once_visible_a_release_can_still_widen_its_rollout() {
        let mut release = release(ReleaseStatus::Upcoming);
        release
            .widen_rollout(at_percent(30), at())
            .expect("nothing has been offered yet");
        release.status = ReleaseStatus::Available;

        release
            .widen_rollout(at_percent(50), later())
            .expect("a real widening");

        assert_eq!(
            release.rollout.percentage(),
            RolloutPercentage::new(50).unwrap()
        );
    }

    /// The aggregate carries the same refusal the rollout type does: nothing
    /// here re-implements or loosens the rule, and it only starts applying
    /// once the release is visible.
    #[test]
    fn once_visible_a_release_refuses_to_narrow_its_own_rollout() {
        let mut release = release(ReleaseStatus::Upcoming);
        release
            .widen_rollout(at_percent(80), at())
            .expect("nothing has been offered yet");
        release.status = ReleaseStatus::Available;

        let error = release
            .widen_rollout(at_percent(10), later())
            .expect_err("narrowing is refused once visible");

        assert!(matches!(
            error,
            RolloutError::PercentageCannotDecrease { .. }
        ));
        assert_eq!(
            release.rollout.percentage(),
            RolloutPercentage::new(80).unwrap(),
            "the refused attempt did not change anything"
        );
    }

    /// A client reading this needs to match on `kind` without also needing to
    /// know which variant carries which fields, which is what a plain
    /// externally-tagged enum would otherwise force.
    #[test]
    fn ineligibility_reasons_serialise_with_a_kind_tag() {
        let reason = IneligibilityReason::OutsideRollout;

        assert_eq!(
            serde_json::to_string(&reason).expect("serialises"),
            "{\"kind\":\"outside_rollout\"}"
        );
    }
}
