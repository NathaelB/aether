use crate::{
    catalog::{BreakingRisk, ReleaseNotes, ReleaseStatus, Rollout},
    deployments::DeploymentKind,
    version::Version,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnounceReleaseCommand {
    pub kind: DeploymentKind,
    pub version: Version,
    pub risk: BreakingRisk,
    pub notes: ReleaseNotes,
    /// Versions to pass through on the way to this one. Empty means it can be
    /// reached directly.
    pub steps_through: Vec<Version>,
    /// Lowest operator/chart version a data plane must run to host this
    /// release. `None` means any operator may install it.
    pub minimum_operator_version: Option<Version>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviseReleaseCommand {
    pub kind: DeploymentKind,
    pub version: Version,
    pub risk: BreakingRisk,
    pub notes: ReleaseNotes,
    pub steps_through: Vec<Version>,
    pub minimum_operator_version: Option<Version>,
}

/// Widens the rollout of a release. Never a narrowing -- see the module docs
/// on [`Rollout`] for why narrowing has no representation at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidenRolloutCommand {
    pub kind: DeploymentKind,
    pub version: Version,
    pub rollout: Rollout,
}

/// Asks how many deployments a candidate rollout would cover, without saving
/// it. The operator screen's way of answering "how many people is this"
/// before committing to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolloutCoveragePreview {
    pub kind: DeploymentKind,
    pub version: Version,
    pub rollout: Rollout,
}

/// Moving a release forward is its own command rather than a field on a
/// revision. Editing notes and withdrawing a version are different acts with
/// different consequences, and one payload that can do both makes a typo in
/// the notes screen able to pull a release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveReleaseCommand {
    pub kind: DeploymentKind,
    pub version: Version,
    pub status: ReleaseStatus,
}
