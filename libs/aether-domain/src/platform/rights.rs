//! What somebody may do to the installation, as opposed to inside an
//! organisation.
//!
//! Named rights rather than a bitmask, deliberately. `Permissions` next door
//! is a bitmask because it has eighteen of them and they are read on every
//! request; there are four here, they are read rarely, and the console has
//! already been bitten once by a bit index -- `ADMINISTRATOR` sits at bit 63
//! and JavaScript's bitwise operators are 32-bit, so the check had to be
//! written as arithmetic. A `TEXT[]` column costs nothing here and is legible
//! from psql at three in the morning, which is when somebody reads it.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::CoreError;

/// One thing an operator may do.
///
/// Split by what it costs to be wrong. Reading the estate is a privilege;
/// changing the fleet is a bigger one; reaching into a customer's deployment
/// is the one that shows up in their incident review.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PlatformRight {
    /// List the data planes, the deployments, the organisations, the
    /// catalogue. Which tenants exist and what they run.
    ViewEstate,

    /// Register and retire a data plane, publish and move a release.
    /// Infrastructure, and no customer's data.
    OperateFleet,

    /// Anything reaching into a customer's own deployment -- taking a backup
    /// of it, restoring it. The right whose misuse a customer would notice.
    ActOnTenant,

    /// Grant and revoke these rights.
    ///
    /// Its own right rather than a property of holding all the others: being
    /// trusted to operate a fleet is not the same as being trusted to decide
    /// who else may.
    ManageOperators,
}

impl PlatformRight {
    /// Every right there is.
    ///
    /// Walked by the exhaustiveness test below, so a right added without a
    /// name is one nobody can be granted.
    pub const ALL: [Self; 4] = [
        Self::ViewEstate,
        Self::OperateFleet,
        Self::ActOnTenant,
        Self::ManageOperators,
    ];
}

impl fmt::Display for PlatformRight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::ViewEstate => "view_estate",
            Self::OperateFleet => "operate_fleet",
            Self::ActOnTenant => "act_on_tenant",
            Self::ManageOperators => "manage_operators",
        };

        write!(f, "{name}")
    }
}

impl FromStr for PlatformRight {
    type Err = CoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "view_estate" => Ok(Self::ViewEstate),
            "operate_fleet" => Ok(Self::OperateFleet),
            "act_on_tenant" => Ok(Self::ActOnTenant),
            "manage_operators" => Ok(Self::ManageOperators),
            other => Err(CoreError::UnknownPlatformRight {
                value: other.to_string(),
            }),
        }
    }
}

/// What one identity holds.
///
/// A set rather than a list: granting the same right twice is granting it
/// once, and a caller that sent it twice meant it once.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(transparent)]
pub struct PlatformRights(Vec<PlatformRight>);

impl PlatformRights {
    /// Everything. What the installation's first operator is given, and what
    /// nobody else is given without somebody deciding to.
    pub fn everything() -> Self {
        Self::of(PlatformRight::ALL)
    }

    pub fn of(rights: impl IntoIterator<Item = PlatformRight>) -> Self {
        let mut held: Vec<_> = rights.into_iter().collect();
        held.sort();
        held.dedup();

        Self(held)
    }

    pub fn holds(&self, right: PlatformRight) -> bool {
        self.0.contains(&right)
    }

    /// Whether everything in `other` is also held here.
    ///
    /// The question behind "you cannot grant a right you do not hold": an
    /// operator handing out `manage_operators` they were never given would be
    /// promoting themselves through somebody else.
    pub fn covers(&self, other: &Self) -> bool {
        other.0.iter().all(|right| self.holds(*right))
    }

    /// What `other` asks for and this does not hold, for a refusal that names
    /// it rather than saying no.
    pub fn missing_from(&self, other: &Self) -> Vec<PlatformRight> {
        other
            .0
            .iter()
            .filter(|right| !self.holds(**right))
            .copied()
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = PlatformRight> + '_ {
        self.0.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A right with no name is one nobody can be granted, and one that does
    /// not survive the round trip is one the database silently drops.
    #[test]
    fn every_right_round_trips_through_its_name() {
        for right in PlatformRight::ALL {
            assert_eq!(right.to_string().parse::<PlatformRight>().unwrap(), right);
        }
    }

    #[test]
    fn a_name_nobody_grants_is_refused_rather_than_ignored() {
        assert!("root".parse::<PlatformRight>().is_err());
        assert!("".parse::<PlatformRight>().is_err());
    }

    #[test]
    fn granting_the_same_right_twice_grants_it_once() {
        let held = PlatformRights::of([PlatformRight::ViewEstate, PlatformRight::ViewEstate]);

        assert_eq!(held.iter().count(), 1);
    }

    /// The rule that stops an operator promoting themselves through somebody
    /// else: they hand out what they hold, and nothing more.
    #[test]
    fn holding_less_does_not_cover_asking_for_more() {
        let reader = PlatformRights::of([PlatformRight::ViewEstate]);
        let asked = PlatformRights::of([PlatformRight::ViewEstate, PlatformRight::ManageOperators]);

        assert!(!reader.covers(&asked));
        assert_eq!(
            reader.missing_from(&asked),
            vec![PlatformRight::ManageOperators],
            "the refusal names what is missing"
        );
    }

    #[test]
    fn everything_covers_every_right() {
        for right in PlatformRight::ALL {
            assert!(PlatformRights::everything().holds(right), "{right}");
        }
    }
}
