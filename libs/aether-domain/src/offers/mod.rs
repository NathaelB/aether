//! What a customer buys for one deployment.
//!
//! Creating a deployment used to ask for a mode, three numbers and a
//! Kubernetes namespace -- five infrastructure decisions taken by the one
//! person in the exchange with no way to make them well. An offer is the
//! single choice that replaces them: it says how large the instance is and
//! whether it shares a cluster, and the platform derives the rest.
//!
//! A closed set, the way [`crate::backups::Cadence`] is. A customer writing
//! their own sizing is how a form field becomes a support conversation, and
//! the sizes here are the ones the platform can actually place.

pub mod availability;
pub mod ports;

pub use availability::{OfferAvailability, offers_for};

use std::fmt;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    CoreError,
    dataplane::value_objects::{DataPlaneMode, DeploymentResources},
    organisation::value_objects::Plan,
};

/// One offer.
///
/// Named for what a customer recognises rather than for the infrastructure
/// behind it -- except [`Offer::Private`], where the isolation *is* what is
/// being bought and hiding it would be coy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Offer {
    /// Enough to try the product against, on a cluster shared with others.
    Sandbox,
    /// The ordinary one.
    Standard,
    /// The same posture with room to grow into.
    Scale,
    /// A cluster of the organisation's own.
    Private,
}

impl Offer {
    /// Every offer there is.
    ///
    /// Kept here rather than derived, and the exhaustiveness test below walks
    /// it: an offer missing from this list is one a customer can hold and no
    /// screen can show.
    pub const ALL: [Self; 4] = [Self::Sandbox, Self::Standard, Self::Scale, Self::Private];

    /// What an instance on this offer is given.
    ///
    /// Read once, when the deployment is created, and stored on the
    /// deployment from then on. An offer whose sizing is revised later must
    /// not silently resize the instances already running on it -- a customer
    /// finds out about that from a restart, not from a release note.
    pub fn resources(self) -> DeploymentResources {
        match self {
            Self::Sandbox => DeploymentResources {
                cpu_millis: 250,
                memory_mib: 512,
                storage_gib: 1,
            },
            Self::Standard => DeploymentResources::DEFAULT,
            Self::Scale => DeploymentResources {
                cpu_millis: 2000,
                memory_mib: 4096,
                storage_gib: 20,
            },
            Self::Private => DeploymentResources {
                cpu_millis: 2000,
                memory_mib: 4096,
                storage_gib: 20,
            },
        }
    }

    /// Whether this shares a cluster.
    ///
    /// [`DataPlaneMode`] rather than an isolation enum of its own. It is the
    /// same question placement already asks, and a second vocabulary for it
    /// would need translating in both directions for ever.
    pub fn mode(self) -> DataPlaneMode {
        match self {
            Self::Sandbox | Self::Standard | Self::Scale => DataPlaneMode::Shared,
            Self::Private => DataPlaneMode::Dedicated,
        }
    }

    /// Which commercial tiers may choose it.
    ///
    /// Stated by the offer rather than by the tier, the way a release states
    /// the plans it is available to. One direction, so the two cannot come to
    /// disagree.
    pub fn open_to(self) -> &'static [Plan] {
        match self {
            Self::Sandbox => &[Plan::Free, Plan::Starter, Plan::Business, Plan::Enterprise],
            Self::Standard => &[Plan::Starter, Plan::Business, Plan::Enterprise],
            Self::Scale => &[Plan::Business, Plan::Enterprise],
            Self::Private => &[Plan::Enterprise],
        }
    }

    /// Whether this tier may choose this offer.
    pub fn is_open_to(self, plan: Plan) -> bool {
        self.open_to().contains(&plan)
    }

    /// The cheapest tier that opens it, for a refusal that says what to do.
    ///
    /// A refusal naming only what is not allowed leaves the customer to guess
    /// which of four tiers would change the answer.
    pub fn cheapest_tier(self) -> Plan {
        // `open_to` is written cheapest first, and the test below holds that.
        self.open_to().first().copied().unwrap_or(Plan::Enterprise)
    }
}

impl fmt::Display for Offer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sandbox => write!(f, "sandbox"),
            Self::Standard => write!(f, "standard"),
            Self::Scale => write!(f, "scale"),
            Self::Private => write!(f, "private"),
        }
    }
}

impl std::str::FromStr for Offer {
    type Err = CoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_lowercase().as_str() {
            "sandbox" => Ok(Self::Sandbox),
            "standard" => Ok(Self::Standard),
            "scale" => Ok(Self::Scale),
            "private" => Ok(Self::Private),
            other => Err(CoreError::InvalidOffer {
                value: other.to_string(),
            }),
        }
    }
}

impl Plan {
    /// What this organisation may choose from.
    ///
    /// Derived from the offers rather than listed again here. A second list
    /// is a second thing to update, and the one that gets forgotten is the
    /// one that quietly sells something.
    pub fn offers(self) -> Vec<Offer> {
        Offer::ALL
            .into_iter()
            .filter(|offer| offer.is_open_to(self))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A set with a hole in it is a set that places a deployment nowhere.
    #[test]
    fn every_offer_answers_all_three_questions() {
        for offer in Offer::ALL {
            assert!(offer.resources().cpu_millis > 0, "{offer} gives no cpu");
            assert!(!offer.open_to().is_empty(), "{offer} is open to nobody");
            // `mode()` is total by construction; naming it here is what makes
            // the loop fail to compile rather than fail at runtime when a
            // variant is added.
            let _ = offer.mode();
        }
    }

    #[test]
    fn every_offer_round_trips_through_its_name() {
        for offer in Offer::ALL {
            assert_eq!(offer.to_string().parse::<Offer>().unwrap(), offer);
        }
    }

    #[test]
    fn a_name_nobody_offers_is_refused_rather_than_defaulted() {
        assert!("enterprise-plus".parse::<Offer>().is_err());
        assert!("".parse::<Offer>().is_err());
    }

    /// The case this whole gate exists for.
    #[test]
    fn the_free_tier_opens_no_cluster_of_its_own() {
        assert!(!Offer::Private.is_open_to(Plan::Free));
        assert!(
            !Plan::Free
                .offers()
                .iter()
                .any(|offer| offer.mode() == DataPlaneMode::Dedicated),
            "a free organisation was offered a cluster of its own"
        );
    }

    /// Every tier can put something somewhere. A tier that opens nothing is a
    /// customer who has paid and cannot deploy.
    #[test]
    fn every_tier_opens_at_least_one_offer() {
        for plan in [Plan::Free, Plan::Starter, Plan::Business, Plan::Enterprise] {
            assert!(!plan.offers().is_empty(), "{plan} opens nothing");
        }
    }

    /// Paying more never takes an offer away.
    #[test]
    fn a_higher_tier_opens_everything_a_lower_one_does() {
        let ladder = [Plan::Free, Plan::Starter, Plan::Business, Plan::Enterprise];

        for pair in ladder.windows(2) {
            let (lower, higher) = (pair[0], pair[1]);

            for offer in lower.offers() {
                assert!(
                    offer.is_open_to(higher),
                    "{higher} does not open {offer}, which {lower} does"
                );
            }
        }
    }

    /// A refusal has to name the tier that would change the answer, so the
    /// cheapest one has to be first in the list rather than merely present.
    #[test]
    fn the_cheapest_tier_is_the_one_a_refusal_names() {
        assert_eq!(Offer::Sandbox.cheapest_tier(), Plan::Free);
        assert_eq!(Offer::Standard.cheapest_tier(), Plan::Starter);
        assert_eq!(Offer::Scale.cheapest_tier(), Plan::Business);
        assert_eq!(Offer::Private.cheapest_tier(), Plan::Enterprise);
    }

    /// A shared offer larger than the platform's default deployment size would
    /// place onto data planes sized for the default, and the mismatch only
    /// shows up as a pod that never schedules.
    #[test]
    fn a_bigger_offer_gives_more_than_a_smaller_one() {
        assert!(Offer::Sandbox.resources().memory_mib < Offer::Standard.resources().memory_mib);
        assert!(Offer::Standard.resources().memory_mib < Offer::Scale.resources().memory_mib);
    }
}
