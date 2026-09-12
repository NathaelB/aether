//! Who may reach a deployment.
//!
//! The rule this module exists to make unbreakable: an allow list that
//! restricts to nothing does not exist. "Open to everyone" and "restricted to
//! nobody" are one keystroke apart in a form, they read almost the same in a
//! database row, and one of them takes a customer's identity provider off the
//! air. So they are two different shapes rather than two values of one.

use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use utoipa::ToSchema;

/// A range of source addresses, in CIDR notation.
///
/// Parsed rather than carried as a string: the platform hands these to an edge
/// proxy, and a value that turns out not to be a CIDR only fails there, long
/// after whoever wrote it has moved on.
/// Carries no schema of its own: on the wire a range is a string, and that is
/// declared once on [`AllowList`] rather than on every type that holds one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Cidr {
    network: IpAddr,
    prefix: u8,
}

impl Cidr {
    pub fn network(&self) -> IpAddr {
        self.network
    }

    pub fn prefix(&self) -> u8 {
        self.prefix
    }
}

impl FromStr for Cidr {
    type Err = NetworkAccessError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let raw = raw.trim();
        let invalid = || NetworkAccessError::NotACidr {
            value: raw.to_string(),
        };

        let (address, prefix) = raw.split_once('/').ok_or_else(invalid)?;
        let address: IpAddr = address.parse().map_err(|_| invalid())?;
        let prefix: u8 = prefix.parse().map_err(|_| invalid())?;

        let width = match address {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };
        if prefix > width {
            return Err(NetworkAccessError::PrefixTooLong {
                value: raw.to_string(),
                prefix,
                width,
            });
        }

        // A prefix with host bits still set is almost always a typo for the
        // network containing that address -- somebody pasted their own IP and
        // widened the mask. Masking it silently would apply a rule nobody
        // wrote, so it is refused, and the message names what they meant.
        let masked = mask(address, prefix);
        if masked != address {
            return Err(NetworkAccessError::HostBitsSet {
                value: raw.to_string(),
                network: format!("{masked}/{prefix}"),
            });
        }

        Ok(Self {
            network: address,
            prefix,
        })
    }
}

fn mask(address: IpAddr, prefix: u8) -> IpAddr {
    match address {
        IpAddr::V4(address) => {
            let bits = u32::from(address);
            let kept = if prefix == 0 {
                0
            } else {
                u32::MAX << (32 - prefix)
            };
            IpAddr::V4((bits & kept).into())
        }
        IpAddr::V6(address) => {
            let bits = u128::from(address);
            let kept = if prefix == 0 {
                0
            } else {
                u128::MAX << (128 - prefix)
            };
            IpAddr::V6((bits & kept).into())
        }
    }
}

impl fmt::Display for Cidr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.network, self.prefix)
    }
}

impl Serialize for Cidr {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Cidr {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(de::Error::custom)
    }
}

/// The ranges a restricted deployment answers. Never empty.
///
/// The emptiness check lives in the constructor rather than in the callers
/// because there is no caller that wants it: a list with nothing in it is a
/// deployment nobody can reach, which is [`NetworkAccess::Open`]'s opposite
/// and not a thing anyone asks for on purpose.
#[derive(Debug, Clone, PartialEq, Eq, ToSchema)]
#[schema(value_type = Vec<String>)]
pub struct AllowList(Vec<Cidr>);

impl AllowList {
    pub fn new(ranges: impl IntoIterator<Item = Cidr>) -> Result<Self, NetworkAccessError> {
        // Duplicates are a mistake rather than an intent, and the first
        // position is kept so the list reads back in the order it was written.
        let mut kept: Vec<Cidr> = Vec::new();
        for range in ranges {
            if !kept.contains(&range) {
                kept.push(range);
            }
        }

        if kept.is_empty() {
            return Err(NetworkAccessError::RestrictedToNothing);
        }

        Ok(Self(kept))
    }

    pub fn ranges(&self) -> &[Cidr] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Always false. Kept so a caller reaching for it finds the answer rather
    /// than the absence of the method.
    pub fn is_empty(&self) -> bool {
        false
    }
}

impl Serialize for AllowList {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AllowList {
    /// Through the constructor, so a payload cannot carry what the type
    /// forbids. Deriving this would let an empty list in over the wire and
    /// leave the guarantee true only for code that goes through Rust.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let ranges = Vec::<Cidr>::deserialize(deserializer)?;
        AllowList::new(ranges).map_err(de::Error::custom)
    }
}

/// Who may reach a deployment.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NetworkAccess {
    /// Reachable from anywhere. What a deployment is until someone says
    /// otherwise.
    #[default]
    Open,
    Restricted {
        allowed: AllowList,
    },
}

impl NetworkAccess {
    /// Builds from a set of ranges, where nothing at all means open.
    ///
    /// The one place the two shapes meet. A form that removes its last entry
    /// arrives here as an empty list and comes back as `Open`, which is what
    /// the person clearing it meant and the only reading that leaves the
    /// deployment reachable.
    pub fn from_ranges(ranges: Vec<Cidr>) -> Result<Self, NetworkAccessError> {
        if ranges.is_empty() {
            return Ok(Self::Open);
        }

        Ok(Self::Restricted {
            allowed: AllowList::new(ranges)?,
        })
    }

    /// The ranges as stored, or nothing when open.
    pub fn ranges(&self) -> &[Cidr] {
        match self {
            Self::Open => &[],
            Self::Restricted { allowed } => allowed.ranges(),
        }
    }

    pub fn is_open(&self) -> bool {
        matches!(self, Self::Open)
    }

    /// Whether an address is allowed through.
    ///
    /// The platform does not enforce this -- the edge does. It is here so the
    /// rule can be stated and tested once, and so a screen can warn someone
    /// before they lock themselves out.
    pub fn admits(&self, address: IpAddr) -> bool {
        match self {
            Self::Open => true,
            Self::Restricted { allowed } => allowed
                .ranges()
                .iter()
                .any(|range| contains(range, address)),
        }
    }
}

fn contains(range: &Cidr, address: IpAddr) -> bool {
    match (range.network(), address) {
        (IpAddr::V4(_), IpAddr::V4(_)) | (IpAddr::V6(_), IpAddr::V6(_)) => {
            mask(address, range.prefix()) == range.network()
        }
        // A v4 rule says nothing about a v6 client. Treating a family
        // mismatch as a match would let an allow list of v4 ranges admit
        // every v6 address on the internet.
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NetworkAccessError {
    #[error("'{value}' is not a CIDR range: expected something like 203.0.113.0/24")]
    NotACidr { value: String },

    #[error("'{value}' has a /{prefix} prefix, but an address of this family stops at /{width}")]
    PrefixTooLong {
        value: String,
        prefix: u8,
        width: u8,
    },

    #[error("'{value}' is not a network: did you mean {network}?")]
    HostBitsSet { value: String, network: String },

    /// The state the type exists to prevent. Reachable only through
    /// [`AllowList::new`], which is why nothing but that constructor can build
    /// one.
    #[error("an allow list cannot be empty: a deployment reachable by nobody is not a setting")]
    RestrictedToNothing,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cidr(raw: &str) -> Cidr {
        raw.parse().expect("a valid range")
    }

    fn ip(raw: &str) -> IpAddr {
        raw.parse().expect("an address")
    }

    /// The rule the whole module exists for. If this ever passes, a form that
    /// clears its last entry can write a deployment nobody can reach.
    #[test]
    fn an_allow_list_cannot_be_empty() {
        let error = AllowList::new(Vec::new()).expect_err("nothing to allow");

        assert_eq!(error, NetworkAccessError::RestrictedToNothing);
    }

    /// The same moment, read the way the person doing it means it. Removing
    /// the last range is going back to open, not restricting to nobody.
    #[test]
    fn clearing_every_range_is_going_back_to_open() {
        let access = NetworkAccess::from_ranges(Vec::new()).expect("open");

        assert_eq!(access, NetworkAccess::Open);
        assert!(access.is_open());
    }

    #[test]
    fn a_deployment_says_nothing_and_is_open() {
        assert_eq!(NetworkAccess::default(), NetworkAccess::Open);
        assert!(NetworkAccess::Open.admits(ip("203.0.113.9")));
    }

    #[test]
    fn a_range_is_read_back_as_it_was_written() {
        assert_eq!(cidr("203.0.113.0/24").to_string(), "203.0.113.0/24");
        assert_eq!(cidr("2001:db8::/32").to_string(), "2001:db8::/32");
        assert_eq!(cidr("10.0.0.1/32").to_string(), "10.0.0.1/32");
    }

    #[test]
    fn what_is_not_a_range_is_refused_at_the_boundary() {
        for raw in ["", "203.0.113.0", "not/a/range", "203.0.113.0/", "/24"] {
            assert!(
                raw.parse::<Cidr>().is_err(),
                "'{raw}' was accepted as a range"
            );
        }
    }

    #[test]
    fn a_prefix_wider_than_its_address_family_is_refused() {
        let error = "203.0.113.0/33".parse::<Cidr>().expect_err("too long");

        assert!(matches!(error, NetworkAccessError::PrefixTooLong { .. }));
        assert!(error.to_string().contains("/32"), "{error}");
    }

    /// Somebody pasting their own address and widening the mask. Masking it
    /// quietly would apply a rule a good deal larger than the one they wrote.
    #[test]
    fn an_address_with_host_bits_left_over_is_refused_and_named() {
        let error = "203.0.113.9/24".parse::<Cidr>().expect_err("host bits");

        assert_eq!(
            error,
            NetworkAccessError::HostBitsSet {
                value: "203.0.113.9/24".to_string(),
                network: "203.0.113.0/24".to_string(),
            }
        );
    }

    #[test]
    fn a_range_written_twice_is_kept_once_in_the_order_it_was_written() {
        let list = AllowList::new(vec![
            cidr("10.0.0.0/8"),
            cidr("203.0.113.0/24"),
            cidr("10.0.0.0/8"),
        ])
        .expect("a list");

        assert_eq!(list.len(), 2);
        assert_eq!(list.ranges()[0].to_string(), "10.0.0.0/8");
        assert_eq!(list.ranges()[1].to_string(), "203.0.113.0/24");
    }

    #[test]
    fn a_restricted_deployment_admits_what_its_ranges_cover() {
        let access = NetworkAccess::from_ranges(vec![cidr("203.0.113.0/24")]).expect("restricted");

        assert!(access.admits(ip("203.0.113.1")));
        assert!(access.admits(ip("203.0.113.255")));
        assert!(!access.admits(ip("203.0.114.1")));
        assert!(!access.admits(ip("198.51.100.1")));
    }

    /// A /0 is written by someone who means "everyone" and should behave that
    /// way rather than matching nothing through an off-by-one in the shift.
    #[test]
    fn a_zero_prefix_admits_everything_of_its_family() {
        let access = NetworkAccess::from_ranges(vec![cidr("0.0.0.0/0")]).expect("restricted");

        assert!(access.admits(ip("203.0.113.1")));
        assert!(access.admits(ip("10.0.0.1")));
    }

    /// A list of v4 ranges saying nothing about v6 must not be read as
    /// admitting every v6 address there is.
    #[test]
    fn a_v4_range_admits_no_v6_address() {
        let access = NetworkAccess::from_ranges(vec![cidr("0.0.0.0/0")]).expect("restricted");

        assert!(!access.admits(ip("2001:db8::1")));
    }

    #[test]
    fn a_v6_range_covers_its_own_family() {
        let access = NetworkAccess::from_ranges(vec![cidr("2001:db8::/32")]).expect("restricted");

        assert!(access.admits(ip("2001:db8::1")));
        assert!(!access.admits(ip("2001:db9::1")));
        assert!(!access.admits(ip("203.0.113.1")));
    }

    #[test]
    fn an_open_deployment_has_no_ranges_to_show() {
        assert!(NetworkAccess::Open.ranges().is_empty());
    }

    #[test]
    fn a_range_survives_a_round_trip_through_json() {
        let access = NetworkAccess::from_ranges(vec![cidr("10.0.0.0/8"), cidr("2001:db8::/32")])
            .expect("restricted");

        let json = serde_json::to_string(&access).expect("serialised");
        let back: NetworkAccess = serde_json::from_str(&json).expect("deserialised");

        assert_eq!(back, access);
    }

    /// The type is the guarantee, but it travels as JSON. A payload carrying
    /// an empty restricted list has to be refused on the way in too.
    #[test]
    fn json_cannot_smuggle_in_an_empty_allow_list() {
        let refused =
            serde_json::from_str::<NetworkAccess>(r#"{"kind":"restricted","allowed":[]}"#);

        assert!(refused.is_err(), "an empty allow list was deserialised");
    }
}
