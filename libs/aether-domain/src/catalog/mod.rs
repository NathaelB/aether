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

use crate::{CoreError, deployments::DeploymentKind, version::Version};

pub mod commands;
pub mod ports;
pub mod service;

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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Release {
    /// A new release starts `Upcoming`.
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

    /// Notes and risk stay editable for as long as the release exists: what a
    /// version breaks is often discovered after it ships, and a catalogue that
    /// cannot record that is worth less than no catalogue.
    pub fn revise(&mut self, risk: BreakingRisk, notes: ReleaseNotes, at: DateTime<Utc>) {
        self.risk = risk;
        self.notes = notes;
        self.updated_at = at;
    }

    pub fn version(&self) -> &Version {
        &self.id.version
    }

    pub fn kind(&self) -> &DeploymentKind {
        &self.id.kind
    }
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
}
