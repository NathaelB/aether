use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use utoipa::ToSchema;

use crate::{
    catalog::ReleaseId, deployments::DeploymentId, organisation::OrganisationId,
    organisation::value_objects::Plan,
};

/// How much of the estate a release is offered to, expressed as 0 to 100.
///
/// A bare `u8` would accept 200; this cannot. Validating on the way in is the
/// same choice [`crate::version::Version`] makes for a version string: an
/// out-of-range percentage is refused at the boundary rather than clamped, so
/// the caller who wrote it finds out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ToSchema)]
#[schema(value_type = u8, example = 10)]
pub struct RolloutPercentage(u8);

impl RolloutPercentage {
    /// Nobody, until raised.
    pub const NONE: Self = Self(0);
    /// Everybody the plan and pilot targeting still allow.
    pub const ALL: Self = Self(100);

    pub fn new(value: u8) -> Result<Self, RolloutError> {
        if value > 100 {
            return Err(RolloutError::PercentageOutOfRange { value });
        }

        Ok(Self(value))
    }

    pub fn value(self) -> u8 {
        self.0
    }
}

impl Serialize for RolloutPercentage {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.0)
    }
}

impl<'de> Deserialize<'de> for RolloutPercentage {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = u8::deserialize(deserializer)?;
        RolloutPercentage::new(raw).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RolloutError {
    #[error("{value} is not a percentage: it must be between 0 and 100")]
    PercentageOutOfRange { value: u8 },

    /// The rule issue #115 exists for: a deployment that already saw this
    /// release offered at 50% must not lose it because someone dialled the
    /// number back to 10. Stopping the input rather than computing around it
    /// means there is no history to reconstruct to know who was already in.
    #[error(
        "the rollout is already at {current}%: it can only be widened, never narrowed to {requested}%"
    )]
    PercentageCannotDecrease { current: u8, requested: u8 },

    /// Symmetrical to the percentage rule, and for the same reason: a plan
    /// already targeted may already have deployments that saw this release as
    /// available, and the catalogue keeps no record of which ones did.
    #[error("{plan} is already targeted by this rollout and cannot be untargeted, only added to")]
    PlanCannotBeUntargeted { plan: String },

    /// Moving from "every plan" to a named set is the same narrowing as
    /// dropping a plan from that set -- there is simply no smaller set than
    /// "everyone" to compare against, so it is refused outright rather than
    /// compared against nothing.
    #[error("this rollout already targets every plan and cannot be restricted to a subset")]
    CannotRestrictFromEveryPlan,
}

/// What a rollout needs to know about the one deployment being evaluated.
///
/// Gathered by the caller rather than looked up here: deciding who a
/// deployment belongs to and what plan its organisation is on is a question
/// for the deployment and organisation contexts, not for the catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RolloutCandidate {
    pub deployment_id: DeploymentId,
    pub organisation_id: OrganisationId,
    pub plan: Plan,
}

/// How much of the estate a release is offered to, beyond being `Available`.
///
/// Three independent knobs, not one: "10% of everyone", "these two plans",
/// and "these two organisations regardless of the other two" are different
/// questions an operator asks at different points in a release's life, and
/// collapsing them into a single filter would make one of them inexpressible
/// alongside the others.
///
/// Every knob here only ever widens once applied -- see [`RolloutError`] for
/// why. The catalogue records no history of who was already offered a
/// release, so "already offered" is answered structurally instead:
/// [`crate::catalog::Release::widen_rollout`] treats a release nobody could
/// have seen yet (still `Upcoming`) as having offered nothing, and lets an
/// operator sculpt its rollout freely up to that point; from the moment a
/// release becomes visible, this type's widen-only rule takes over and
/// nothing offered under it can be taken back. The rejected alternative was
/// recording an offer log per deployment and computing the difference on
/// every narrow -- correct, but a write on every read of the catalogue for a
/// case this installation has not needed yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Rollout {
    percentage: RolloutPercentage,
    /// `None` means every plan. `Some` never contains every possible plan by
    /// construction of [`Rollout::widen_plans`] alone -- nothing stops an
    /// operator listing all four, it simply means the same thing as `None`.
    plans: Option<Vec<Plan>>,
    pilot_organisations: Vec<OrganisationId>,
}

impl Rollout {
    /// No restriction at all: the behaviour before this feature existed.
    /// Every release starts here, so a release that never has its rollout
    /// touched is offered to everyone `Available` already permits.
    pub fn full() -> Self {
        Self {
            percentage: RolloutPercentage::ALL,
            plans: None,
            pilot_organisations: Vec::new(),
        }
    }

    /// Nobody yet.
    ///
    /// Where a release starts: being in the catalogue and being applied to an
    /// estate are two decisions, and publishing should not silently make the
    /// second one. Widening from here is the act that says a version is ready
    /// for everyone, and it only goes this way -- a version somebody has
    /// already taken cannot be un-offered, which is what withdrawing is for.
    pub fn closed() -> Self {
        Self {
            percentage: RolloutPercentage::NONE,
            plans: None,
            pilot_organisations: Vec::new(),
        }
    }

    /// Whether this release is offered to the whole estate, with nothing held
    /// back by percentage or by plan.
    pub fn is_global(&self) -> bool {
        self.percentage == RolloutPercentage::ALL && self.plans.is_none()
    }

    /// Builds a rollout with exactly these values, without the widen-only
    /// checks `widen_percentage` and `widen_plans` enforce.
    ///
    /// Two callers need exactly this: a repository rebuilding a row it
    /// already trusts, and a caller building the *candidate* an operator
    /// submitted so it can be handed to [`crate::catalog::Release::widen_rollout`]
    /// for the actual, checked comparison against what is already stored.
    /// Neither is "applying a novel change" to a rollout already in force --
    /// that check happens exactly once, where the previous and the candidate
    /// are both in hand, not while either one is being constructed.
    ///
    /// The rejected alternative was requiring every candidate to be built by
    /// calling `widen_percentage`/`widen_plans` starting from [`Rollout::full`]:
    /// that conflates "the widest a rollout can ever be" with "a sensible
    /// starting point to grow one from", and a percentage below 100 or a
    /// named plan set would never pass its own construction.
    pub fn new(
        percentage: RolloutPercentage,
        plans: Option<Vec<Plan>>,
        pilot_organisations: Vec<OrganisationId>,
    ) -> Self {
        Self {
            percentage,
            plans,
            pilot_organisations,
        }
    }

    pub fn percentage(&self) -> RolloutPercentage {
        self.percentage
    }

    pub fn plans(&self) -> Option<&[Plan]> {
        self.plans.as_deref()
    }

    pub fn pilot_organisations(&self) -> &[OrganisationId] {
        &self.pilot_organisations
    }

    /// Raises the percentage. Refuses to lower it -- see [`RolloutError`].
    pub fn widen_percentage(&mut self, requested: RolloutPercentage) -> Result<(), RolloutError> {
        if requested < self.percentage {
            return Err(RolloutError::PercentageCannotDecrease {
                current: self.percentage.value(),
                requested: requested.value(),
            });
        }

        self.percentage = requested;
        Ok(())
    }

    /// Grows the targeted plan set, or opens it to every plan.
    ///
    /// `None` always succeeds: "every plan" is the widest possible target, so
    /// moving to it can never take anything away. A `Some` request must
    /// contain every plan already targeted, or it is a narrowing.
    pub fn widen_plans(&mut self, requested: Option<Vec<Plan>>) -> Result<(), RolloutError> {
        let Some(requested) = requested else {
            self.plans = None;
            return Ok(());
        };

        match &self.plans {
            None => Err(RolloutError::CannotRestrictFromEveryPlan),
            Some(current) => {
                for plan in current {
                    if !requested.contains(plan) {
                        return Err(RolloutError::PlanCannotBeUntargeted {
                            plan: plan.to_string(),
                        });
                    }
                }

                let mut widened = current.clone();
                for plan in requested {
                    if !widened.contains(&plan) {
                        widened.push(plan);
                    }
                }
                self.plans = Some(widened);
                Ok(())
            }
        }
    }

    /// Adds pilot organisations. There is no removal: a method that could
    /// only ever add cannot be used to narrow, which is the entire guarantee
    /// -- see [`RolloutError`].
    pub fn add_pilot_organisations(
        &mut self,
        organisations: impl IntoIterator<Item = OrganisationId>,
    ) {
        for organisation_id in organisations {
            if !self.pilot_organisations.contains(&organisation_id) {
                self.pilot_organisations.push(organisation_id);
            }
        }
    }

    /// Whether `candidate` is offered `release` under this rollout.
    ///
    /// Pilots first: a pilot organisation is not merely inside the
    /// percentage, it is not filtered by plan either. Then plan, because it
    /// narrows who the percentage even applies to. The percentage decides
    /// last, over whoever is left.
    pub fn covers(&self, release: &ReleaseId, candidate: &RolloutCandidate) -> bool {
        if self
            .pilot_organisations
            .contains(&candidate.organisation_id)
        {
            return true;
        }

        if let Some(plans) = &self.plans
            && !plans.contains(&candidate.plan)
        {
            return false;
        }

        bucket(release, candidate.deployment_id) < self.percentage.value()
    }
}

/// Deterministic, sticky membership: the same release and the same deployment
/// always land in the same bucket.
///
/// FNV-1a rather than `std::hash::DefaultHasher`: the standard library is
/// explicit that its hasher's algorithm is not fixed across releases, and a
/// bucket that shifts after a toolchain upgrade would silently remove a
/// deployment from a wave it was already in -- precisely what issue #115
/// exists to prevent. FNV-1a has one, unchanging definition, so a rollout
/// evaluated today keeps evaluating the same way next year.
fn bucket(release: &ReleaseId, deployment_id: DeploymentId) -> u8 {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    let mut hash = OFFSET_BASIS;
    for byte in format!("{release}:{deployment_id}").bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }

    (hash % 100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deployments::DeploymentKind;
    use crate::version::Version;
    use uuid::Uuid;

    fn release_id() -> ReleaseId {
        ReleaseId::new(DeploymentKind::Ferriskey, Version::new(26, 0, 1))
    }

    fn candidate() -> RolloutCandidate {
        RolloutCandidate {
            deployment_id: DeploymentId(Uuid::new_v4()),
            organisation_id: OrganisationId(Uuid::new_v4()),
            plan: Plan::Free,
        }
    }

    fn at(percent: u8) -> Rollout {
        Rollout::new(RolloutPercentage::new(percent).unwrap(), None, Vec::new())
    }

    #[test]
    fn a_percentage_over_a_hundred_is_refused() {
        let error = RolloutPercentage::new(101).expect_err("not a percentage");
        assert!(matches!(
            error,
            RolloutError::PercentageOutOfRange { value: 101 }
        ));
    }

    #[test]
    fn zero_and_a_hundred_are_valid_percentages() {
        assert!(RolloutPercentage::new(0).is_ok());
        assert!(RolloutPercentage::new(100).is_ok());
    }

    /// The literal acceptance criterion of #115: two evaluations of the same
    /// deployment give the same answer, because nothing here is a draw.
    #[test]
    fn the_same_deployment_gets_the_same_answer_every_time() {
        let rollout = at(37);
        let release = release_id();
        let candidate = candidate();

        let first = rollout.covers(&release, &candidate);
        let second = rollout.covers(&release, &candidate);
        let third = rollout.covers(&release, &candidate);

        assert_eq!(first, second);
        assert_eq!(second, third);
    }

    /// The other literal acceptance criterion: going from 10% to 50% takes
    /// nobody out of the wave. Walked as one rollout progressively widened,
    /// rather than several independent ones, so the assertion is about the
    /// mutation and not merely about higher percentages covering more.
    #[test]
    fn widening_a_rollout_takes_nobody_out_of_it() {
        let release = release_id();
        let candidates: Vec<RolloutCandidate> = (0..500).map(|_| candidate()).collect();
        let mut rollout = at(0);

        let mut previous_members: Option<Vec<bool>> = None;
        for percent in [10u8, 25, 50, 75, 90, 100] {
            rollout
                .widen_percentage(RolloutPercentage::new(percent).unwrap())
                .expect("each step widens");

            let members: Vec<bool> = candidates
                .iter()
                .map(|candidate| rollout.covers(&release, candidate))
                .collect();

            if let Some(previous) = &previous_members {
                for (was_in, still_in) in previous.iter().zip(members.iter()) {
                    assert!(
                        !was_in || *still_in,
                        "a deployment left the wave as the percentage grew to {percent}"
                    );
                }
            }

            previous_members = Some(members);
        }
    }

    /// Checked on a sample rather than asserted uniform: a hash that clumps
    /// everyone into the first ten buckets would pass a determinism test and
    /// still be a rollout that offers a release to 100% of a "10%" wave.
    #[test]
    fn the_percentage_spreads_across_a_sample_rather_than_clumping() {
        let release = release_id();
        let rollout = at(20);

        let sample_size = 5_000;
        let covered = (0..sample_size)
            .filter(|_| rollout.covers(&release, &candidate()))
            .count();

        let observed_percent = (covered as f64 / sample_size as f64) * 100.0;
        assert!(
            (15.0..=25.0).contains(&observed_percent),
            "expected roughly 20% of a 5000-deployment sample, got {observed_percent}%"
        );
    }

    #[test]
    fn zero_percent_covers_nobody_and_a_hundred_covers_everybody() {
        let release = release_id();

        let none = at(0);
        assert!(!none.covers(&release, &candidate()));

        let full = Rollout::full();
        for _ in 0..50 {
            assert!(full.covers(&release, &candidate()));
        }
    }

    /// The whole point of a pilot: it works even at 0%.
    #[test]
    fn a_pilot_organisation_is_covered_however_small_the_percentage() {
        let release = release_id();
        let organisation_id = OrganisationId(Uuid::new_v4());
        let mut rollout = at(0);
        rollout.add_pilot_organisations([organisation_id]);

        let candidate = RolloutCandidate {
            deployment_id: DeploymentId(Uuid::new_v4()),
            organisation_id,
            plan: Plan::Free,
        };

        assert!(rollout.covers(&release, &candidate));
    }

    #[test]
    fn a_plan_outside_the_target_is_not_covered_even_at_full_percentage() {
        let release = release_id();
        let rollout = Rollout::new(
            RolloutPercentage::ALL,
            Some(vec![Plan::Enterprise]),
            Vec::new(),
        );

        let candidate = RolloutCandidate {
            deployment_id: DeploymentId(Uuid::new_v4()),
            organisation_id: OrganisationId(Uuid::new_v4()),
            plan: Plan::Free,
        };

        assert!(!rollout.covers(&release, &candidate));
    }

    #[test]
    fn a_targeted_plan_is_still_subject_to_the_percentage() {
        let release = release_id();
        let rollout = Rollout::new(
            RolloutPercentage::NONE,
            Some(vec![Plan::Enterprise]),
            Vec::new(),
        );

        let candidate = RolloutCandidate {
            deployment_id: DeploymentId(Uuid::new_v4()),
            organisation_id: OrganisationId(Uuid::new_v4()),
            plan: Plan::Enterprise,
        };

        assert!(!rollout.covers(&release, &candidate));
    }

    #[test]
    fn narrowing_the_percentage_is_refused() {
        let mut rollout = at(50);

        let error = rollout
            .widen_percentage(RolloutPercentage::new(10).unwrap())
            .expect_err("percentage never decreases");

        assert!(matches!(
            error,
            RolloutError::PercentageCannotDecrease {
                current: 50,
                requested: 10
            }
        ));
        assert_eq!(rollout.percentage(), RolloutPercentage::new(50).unwrap());
    }

    #[test]
    fn widening_the_percentage_to_the_same_value_is_allowed() {
        let mut rollout = at(50);

        rollout
            .widen_percentage(RolloutPercentage::new(50).unwrap())
            .expect("holding steady is not a narrowing");
    }

    #[test]
    fn removing_a_targeted_plan_is_refused() {
        let mut rollout = Rollout::new(
            RolloutPercentage::ALL,
            Some(vec![Plan::Business, Plan::Enterprise]),
            Vec::new(),
        );

        let error = rollout
            .widen_plans(Some(vec![Plan::Enterprise]))
            .expect_err("business was already targeted");

        assert!(matches!(error, RolloutError::PlanCannotBeUntargeted { .. }));
        assert_eq!(
            rollout.plans(),
            Some([Plan::Business, Plan::Enterprise].as_slice())
        );
    }

    #[test]
    fn adding_a_plan_to_the_target_keeps_the_ones_already_there() {
        let mut rollout = Rollout::new(
            RolloutPercentage::ALL,
            Some(vec![Plan::Business]),
            Vec::new(),
        );

        rollout
            .widen_plans(Some(vec![Plan::Business, Plan::Enterprise]))
            .expect("a superset is a widening");

        assert_eq!(
            rollout.plans(),
            Some([Plan::Business, Plan::Enterprise].as_slice())
        );
    }

    #[test]
    fn restricting_from_every_plan_to_a_subset_is_refused() {
        let mut rollout = Rollout::full();
        assert_eq!(rollout.plans(), None);

        let error = rollout
            .widen_plans(Some(vec![Plan::Enterprise]))
            .expect_err("every plan is wider than any subset");

        assert!(matches!(error, RolloutError::CannotRestrictFromEveryPlan));
    }

    #[test]
    fn opening_a_restricted_rollout_to_every_plan_is_allowed() {
        let mut rollout = Rollout::new(
            RolloutPercentage::ALL,
            Some(vec![Plan::Enterprise]),
            Vec::new(),
        );

        rollout
            .widen_plans(None)
            .expect("every plan is wider than any subset");
        assert_eq!(rollout.plans(), None);
    }

    /// There is no method that can remove a pilot: the guarantee comes from
    /// the shape of the API, not from a check inside it.
    #[test]
    fn adding_the_same_pilot_twice_does_not_duplicate_it() {
        let organisation_id = OrganisationId(Uuid::new_v4());
        let mut rollout = Rollout::full();

        rollout.add_pilot_organisations([organisation_id]);
        rollout.add_pilot_organisations([organisation_id]);

        assert_eq!(rollout.pilot_organisations(), [organisation_id]);
    }

    #[test]
    fn percentages_serialise_as_a_plain_number() {
        let percentage = RolloutPercentage::new(42).unwrap();
        assert_eq!(serde_json::to_string(&percentage).unwrap(), "42");
    }

    #[test]
    fn a_percentage_over_a_hundred_is_refused_on_deserialisation_too() {
        let error =
            serde_json::from_str::<RolloutPercentage>("142").expect_err("142 is not a percentage");
        assert!(error.to_string().contains("142"));
    }
}
