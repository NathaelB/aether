//! The version of an identity provider, as something that can be compared.
//!
//! It used to be a free string, holding `"latest"` in some rows and `"1.0.0"`
//! in others. Nothing could compare two of them, so nothing could tell a patch
//! from a major -- which is the distinction every upgrade decision rests on.
//!
//! Parsing is delegated to `semver`, whose ordering rules are the ones the
//! ecosystem already agrees on. What lives here is the boundary: what the
//! domain accepts, and what it says when it refuses.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use utoipa::ToSchema;

/// A released version, e.g. `26.0.1`.
///
/// Ordered by semver rules rather than lexically, so `26.0.10` comes after
/// `26.0.9` and a prerelease comes before the release it leads to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, ToSchema)]
#[schema(value_type = String, example = "26.0.1")]
pub struct Version(semver::Version);

/// What separates one version from another, looking forward.
///
/// The pivot of the client upgrade policy: a patch can be applied without
/// asking, a major never can. Deriving it from the numbers is only safe
/// because [`Version`] parses them; on the free string it replaced, this
/// question had no answer at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum VersionChange {
    Patch,
    Minor,
    Major,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VersionError {
    /// Kept apart from the general parse failure because it is not a typo: it
    /// is a deliberate choice the platform used to accept, and the person who
    /// wrote it needs to be told what to write instead.
    #[error(
        "'latest' is not a version: a deployment records the version it runs, so pass an exact one such as 26.0.1"
    )]
    Floating,

    #[error("'{input}' is not a version: expected major.minor.patch, such as 26.0.1")]
    Malformed { input: String },

    #[error("{to} is not ahead of {from}, and there is no downgrade")]
    NotAhead { from: String, to: String },
}

impl Version {
    pub fn parse(input: &str) -> Result<Self, VersionError> {
        let trimmed = input.trim();

        if trimmed.eq_ignore_ascii_case("latest") {
            return Err(VersionError::Floating);
        }

        semver::Version::parse(trimmed)
            .map(Self)
            .map_err(|_| VersionError::Malformed {
                input: input.to_string(),
            })
    }

    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self(semver::Version::new(major, minor, patch))
    }

    pub fn major(&self) -> u64 {
        self.0.major
    }

    pub fn minor(&self) -> u64 {
        self.0.minor
    }

    pub fn patch(&self) -> u64 {
        self.0.patch
    }

    pub fn is_prerelease(&self) -> bool {
        !self.0.pre.is_empty()
    }

    /// How far `target` is from this version.
    ///
    /// A target that is not ahead has no classification, because there is no
    /// such thing as a backwards upgrade: the answer is a refusal, not a
    /// fourth variant. Keeping it out of [`VersionChange`] means a caller
    /// cannot forget to handle it.
    pub fn change_to(&self, target: &Version) -> Result<VersionChange, VersionError> {
        if target <= self {
            return Err(VersionError::NotAhead {
                from: self.to_string(),
                to: target.to_string(),
            });
        }

        Ok(if target.major() != self.major() {
            VersionChange::Major
        } else if target.minor() != self.minor() {
            VersionChange::Minor
        } else {
            VersionChange::Patch
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Version {
    type Err = VersionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// Serialised as the plain string it has always been, so no API contract
/// changes. Deserialisation is where the validation happens, which is the
/// point: an unparsable version cannot enter the domain through a payload.
impl Serialize for Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Version::parse(&raw).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_released_version() {
        let version = Version::parse("26.0.1").expect("a released version");

        assert_eq!(version.major(), 26);
        assert_eq!(version.minor(), 0);
        assert_eq!(version.patch(), 1);
    }

    /// The whole reason this type exists. Lexically, "26.0.9" sorts after
    /// "26.0.10", which would offer an upgrade backwards.
    #[test]
    fn orders_numerically_not_lexically() {
        let nine = Version::parse("26.0.9").expect("valid");
        let ten = Version::parse("26.0.10").expect("valid");

        assert!(ten > nine, "26.0.10 must come after 26.0.9");
        assert!("26.0.10" < "26.0.9", "the string comparison this replaces");
    }

    #[test]
    fn orders_across_components() {
        let mut versions = [
            Version::parse("26.1.0").expect("valid"),
            Version::parse("25.9.9").expect("valid"),
            Version::parse("27.0.0").expect("valid"),
            Version::parse("26.0.5").expect("valid"),
        ];
        versions.sort();

        let sorted: Vec<String> = versions.iter().map(|v| v.to_string()).collect();
        assert_eq!(sorted, ["25.9.9", "26.0.5", "26.1.0", "27.0.0"]);
    }

    /// A prerelease leads to its release, so it has to sort before it.
    /// Offering 26.0.0 as an upgrade from 26.0.0-rc1 depends on this.
    #[test]
    fn a_prerelease_comes_before_its_release() {
        let candidate = Version::parse("26.0.0-rc.1").expect("valid");
        let released = Version::parse("26.0.0").expect("valid");

        assert!(candidate < released);
        assert!(candidate.is_prerelease());
        assert!(!released.is_prerelease());
    }

    /// Prereleases of the same version order among themselves, so an rc.2 is
    /// an upgrade from an rc.1 rather than an equal.
    #[test]
    fn prereleases_order_among_themselves() {
        let first = Version::parse("26.0.0-rc.1").expect("valid");
        let second = Version::parse("26.0.0-rc.2").expect("valid");

        assert!(second > first);
    }

    /// The value the platform accepted until now. Refusing it silently would
    /// leave the caller guessing, so the message names the replacement.
    #[test]
    fn latest_is_refused_and_says_what_to_write_instead() {
        let error = Version::parse("latest").expect_err("floating versions are refused");

        assert_eq!(error, VersionError::Floating);
        assert!(error.to_string().contains("26.0.1"), "{error}");
    }

    #[test]
    fn latest_is_refused_whatever_its_casing() {
        for input in ["latest", "LATEST", "Latest", "  latest  "] {
            assert_eq!(
                Version::parse(input),
                Err(VersionError::Floating),
                "{input} must be refused"
            );
        }
    }

    #[test]
    fn refuses_what_is_not_a_version() {
        for input in ["", "26", "26.0", "v26.0.1", "26.0.x", "nightly"] {
            let error = Version::parse(input).expect_err("not a version");

            assert!(
                matches!(error, VersionError::Malformed { .. }),
                "{input} produced {error:?}"
            );
            assert!(
                error.to_string().contains(input),
                "the message repeats what was passed: {error}"
            );
        }
    }

    #[test]
    fn surrounding_whitespace_is_forgiven() {
        assert_eq!(
            Version::parse(" 26.0.1 ").expect("valid"),
            Version::parse("26.0.1").expect("valid")
        );
    }

    /// The API contract does not move: it was a string before and it stays one.
    #[test]
    fn serialises_as_a_plain_string() {
        let version = Version::parse("26.0.1").expect("valid");

        assert_eq!(
            serde_json::to_string(&version).expect("serialises"),
            "\"26.0.1\""
        );
    }

    #[test]
    fn deserialises_from_a_plain_string() {
        let version: Version = serde_json::from_str("\"26.0.1\"").expect("deserialises");

        assert_eq!(version, Version::parse("26.0.1").expect("valid"));
    }

    /// Validation on the way in is what stops an unusable version reaching the
    /// domain through a payload, rather than being noticed later.
    #[test]
    fn deserialisation_refuses_a_bad_version() {
        let error = serde_json::from_str::<Version>("\"latest\"")
            .expect_err("floating versions are refused on the way in too");

        assert!(error.to_string().contains("latest"), "{error}");
    }

    #[test]
    fn round_trips_through_its_own_text() {
        for input in ["26.0.1", "1.0.0-rc.1", "0.0.1"] {
            let version = Version::parse(input).expect("valid");

            assert_eq!(version.to_string(), input);
            assert_eq!(
                Version::parse(&version.to_string()).expect("valid"),
                version
            );
        }
    }

    fn v(text: &str) -> Version {
        Version::parse(text).expect("valid")
    }

    #[test]
    fn classifies_the_three_kinds_of_step() {
        assert_eq!(
            v("26.0.0").change_to(&v("26.0.1")),
            Ok(VersionChange::Patch)
        );
        assert_eq!(
            v("26.0.0").change_to(&v("26.1.0")),
            Ok(VersionChange::Minor)
        );
        assert_eq!(
            v("26.0.0").change_to(&v("27.0.0")),
            Ok(VersionChange::Major)
        );
    }

    /// A jump that crosses a zero is where a naive component-by-component
    /// comparison goes wrong: the patch number goes down while the step is
    /// forward.
    #[test]
    fn a_step_across_a_zero_is_read_by_the_component_that_moved() {
        assert_eq!(
            v("26.0.9").change_to(&v("26.1.0")),
            Ok(VersionChange::Minor)
        );
        assert_eq!(
            v("26.9.9").change_to(&v("27.0.0")),
            Ok(VersionChange::Major)
        );
    }

    /// The widest component wins. Going from 26.0.0 to 27.5.3 is a major, not
    /// three separate steps, and the policy has to see it as one.
    #[test]
    fn the_widest_component_decides() {
        assert_eq!(
            v("26.0.0").change_to(&v("27.5.3")),
            Ok(VersionChange::Major)
        );
        assert_eq!(
            v("26.0.0").change_to(&v("26.5.3")),
            Ok(VersionChange::Minor)
        );
    }

    /// There is no downgrade, so a target that is not ahead is refused rather
    /// than classified. Kept out of the enum so a caller cannot forget it.
    #[test]
    fn a_target_that_is_not_ahead_is_refused() {
        let error = v("26.1.0")
            .change_to(&v("26.0.9"))
            .expect_err("there is no downgrade");

        assert!(matches!(error, VersionError::NotAhead { .. }));
        assert!(error.to_string().contains("26.0.9"), "{error}");
        assert!(error.to_string().contains("26.1.0"), "{error}");
    }

    #[test]
    fn a_version_is_not_a_step_from_itself() {
        assert!(matches!(
            v("26.0.1").change_to(&v("26.0.1")),
            Err(VersionError::NotAhead { .. })
        ));
    }

    /// Leaving a prerelease for its release is a step forward, and the
    /// smallest one: nothing in the numbers changed.
    #[test]
    fn leaving_a_prerelease_is_a_patch() {
        assert_eq!(
            v("26.0.0-rc.1").change_to(&v("26.0.0")),
            Ok(VersionChange::Patch)
        );
    }

    /// Going back into a prerelease is going backwards, whatever the numbers
    /// look like.
    #[test]
    fn entering_a_prerelease_of_the_same_version_is_refused() {
        assert!(matches!(
            v("26.0.0").change_to(&v("26.0.0-rc.1")),
            Err(VersionError::NotAhead { .. })
        ));
    }

    #[test]
    fn serialises_in_the_same_case_as_the_rest_of_the_api() {
        assert_eq!(
            serde_json::to_string(&VersionChange::Major).expect("serialises"),
            "\"major\""
        );
    }
}
