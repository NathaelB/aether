use crate::{
    catalog::{BreakingRisk, ReleaseNotes, ReleaseStatus},
    deployments::DeploymentKind,
    version::Version,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnounceReleaseCommand {
    pub kind: DeploymentKind,
    pub version: Version,
    pub risk: BreakingRisk,
    pub notes: ReleaseNotes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviseReleaseCommand {
    pub kind: DeploymentKind,
    pub version: Version,
    pub risk: BreakingRisk,
    pub notes: ReleaseNotes,
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
