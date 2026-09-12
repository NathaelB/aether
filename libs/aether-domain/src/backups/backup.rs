//! What a backup is, and what it can be restored onto.
//!
//! One rule shapes the whole type: **a backup exists only if it completed.**
//! There is no status here and no failed variant. An attempt that did not
//! finish is an entry in `actions` and in the audit log, which is where the
//! rest of the platform already records work that was tried.
//!
//! The alternative looks harmless and is not. A row saying `status: Failed`
//! is a restore somebody will eventually attempt, and every function that
//! reads backups grows a check it can forget. Absence cannot be forgotten.
//!
//! The second rule is that an archive records what it can come back onto. A
//! physical backup is locked to the Postgres major it was taken from, and a
//! restore never goes to an earlier release. Both refusals live here rather
//! than in the restore path, because the restore path runs during an outage
//! and is the worst place to discover a rule.

use std::{fmt, num::NonZeroU64, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    CoreError,
    backups::{ObjectLocation, keys::KeyRef},
    catalog::ReleaseId,
    deployments::DeploymentId,
    organisation::OrganisationId,
    version::Version,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct BackupId(pub Uuid);

impl fmt::Display for BackupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for BackupId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl FromStr for BackupId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::from_str(s).map(BackupId)
    }
}

/// The Postgres major an archive was taken from.
///
/// Carried because a physical backup cannot be restored onto a different one,
/// and because discovering that at restore time means discovering it during an
/// outage. A logical dump does not care, which is why the constraint belongs
/// to the method rather than to every backup.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema,
)]
pub struct PostgresMajor(pub u32);

impl fmt::Display for PostgresMajor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// How the archive was taken, and therefore what it can do on the way back.
///
/// Recorded rather than inferred from size or from the plan. The same
/// deployment can hold archives of both kinds -- a nightly physical one and a
/// weekly portable one -- and what each allows is a property of the archive,
/// not of the deployment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupMethod {
    /// A dump. Slow to restore, and restorable onto any Postgres that can read
    /// it, which is what makes it the one that survives a major upgrade.
    Logical,
    /// A base backup. Fast, and locked to the Postgres major it came from.
    Physical,
}

impl BackupMethod {
    /// Whether an archive of this kind may land on a different Postgres major.
    pub fn crosses_postgres_majors(self) -> bool {
        matches!(self, Self::Logical)
    }
}

impl fmt::Display for BackupMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Logical => write!(f, "logical"),
            Self::Physical => write!(f, "physical"),
        }
    }
}

impl TryFrom<&str> for BackupMethod {
    type Error = CoreError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "logical" => Ok(Self::Logical),
            "physical" => Ok(Self::Physical),
            other => Err(CoreError::InternalError(format!(
                "invalid backup method: {other}"
            ))),
        }
    }
}

/// An archive that exists.
///
/// Every field is present because the archive finished. `size_bytes` is a
/// [`NonZeroU64`] for the same reason there is no status: an archive of zero
/// bytes is not a small archive, it is a backup that did not happen, and the
/// type refuses to describe one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Backup {
    pub id: BackupId,
    pub deployment_id: DeploymentId,
    pub organisation_id: OrganisationId,

    /// What was running when it was taken. A restore never goes backwards from
    /// this.
    pub release: ReleaseId,

    /// What a physical archive is locked to.
    pub postgres_major: PostgresMajor,

    pub method: BackupMethod,

    /// Which key wrapped the data key. Recorded, never resolved at read time:
    /// "the current version" is not an answer to "what encrypted this".
    pub key: KeyRef,

    /// Where the manifest sits. The archive itself is written by the data
    /// plane's Postgres operator under the same prefix.
    pub location: ObjectLocation,

    /// Measured, from the first version of this feature. Quotas are not in
    /// scope yet and a measurement nobody took cannot be backfilled.
    #[schema(value_type = u64, example = 4096)]
    pub size_bytes: NonZeroU64,

    pub started_at: DateTime<Utc>,

    /// Not optional. An archive still being written is not a backup.
    pub finished_at: DateTime<Utc>,
}

/// What a restore is aiming at.
///
/// Gathered by the caller rather than looked up here, the same way
/// [`crate::catalog::RolloutCandidate`] is: what release a deployment would
/// come back on, and what Postgres it would come back on, are questions for
/// the deployment and catalogue contexts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreTarget {
    pub release: ReleaseId,
    pub postgres_major: PostgresMajor,
}

impl Backup {
    /// Whether this archive can be restored onto that target.
    ///
    /// Two refusals, and they are not the same shape. A release that moved
    /// backwards is refused for every method: Postgres will not read a
    /// newer catalogue, and neither will the product's own migrations. A
    /// Postgres major that moved at all is refused only for a physical
    /// archive, because that is the one whose bytes belong to a specific
    /// engine.
    pub fn restorable_onto(&self, target: &RestoreTarget) -> Result<(), CoreError> {
        if self.release.kind != target.release.kind {
            return Err(CoreError::BackupNotRestorable {
                backup: self.id.0,
                reason: format!(
                    "it holds {} and the target runs {}",
                    self.release.kind, target.release.kind
                ),
            });
        }

        if target.release.version < self.release.version {
            return Err(CoreError::BackupFromALaterRelease {
                backup: self.id.0,
                taken_on: self.release.version.to_string(),
                target: target.release.version.to_string(),
            });
        }

        if !self.method.crosses_postgres_majors() && target.postgres_major != self.postgres_major {
            return Err(CoreError::BackupLockedToPostgresMajor {
                backup: self.id.0,
                taken_on: self.postgres_major,
                target: target.postgres_major,
            });
        }

        Ok(())
    }

    /// How long the archive took to produce.
    ///
    /// Kept as a function of the two instants rather than as a stored
    /// duration: two values that can disagree are two values that eventually
    /// will.
    pub fn duration(&self) -> chrono::Duration {
        self.finished_at - self.started_at
    }

    /// The version this archive was taken on, which is the floor for any
    /// restore of it.
    pub fn version(&self) -> &Version {
        &self.release.version
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use chrono::TimeZone;

    use super::*;
    use crate::{
        backups::{
            ArchivePrefix,
            keys::{KeyName, KeyVersion, ProviderName},
        },
        deployments::DeploymentKind,
    };

    pub fn backup(method: BackupMethod, version: &str, postgres_major: u32) -> Backup {
        let organisation = OrganisationId(Uuid::from_u128(1));
        let deployment = DeploymentId(Uuid::from_u128(2));
        let prefix = ArchivePrefix::new(organisation, deployment);

        Backup {
            id: BackupId(Uuid::from_u128(3)),
            deployment_id: deployment,
            organisation_id: organisation,
            release: ReleaseId::new(DeploymentKind::Keycloak, Version::parse(version).unwrap()),
            postgres_major: PostgresMajor(postgres_major),
            method,
            key: KeyRef::new(
                ProviderName::platform(),
                KeyName::new("aether-backups").unwrap(),
                KeyVersion::new(1),
            ),
            location: prefix.object("manifest.json").unwrap(),
            size_bytes: NonZeroU64::new(4096).unwrap(),
            started_at: Utc.with_ymd_and_hms(2026, 9, 12, 2, 0, 0).unwrap(),
            finished_at: Utc.with_ymd_and_hms(2026, 9, 12, 2, 30, 0).unwrap(),
        }
    }

    pub fn target(version: &str, postgres_major: u32) -> RestoreTarget {
        RestoreTarget {
            release: ReleaseId::new(DeploymentKind::Keycloak, Version::parse(version).unwrap()),
            postgres_major: PostgresMajor(postgres_major),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{fixtures::*, *};
    use crate::deployments::DeploymentKind;

    #[test]
    fn an_archive_restores_onto_the_release_it_was_taken_on() {
        let backup = backup(BackupMethod::Physical, "26.0.0", 17);

        assert!(backup.restorable_onto(&target("26.0.0", 17)).is_ok());
    }

    #[test]
    fn an_archive_restores_onto_a_later_release() {
        let backup = backup(BackupMethod::Logical, "26.0.0", 17);

        assert!(backup.restorable_onto(&target("26.1.0", 17)).is_ok());
    }

    #[test]
    fn an_archive_does_not_restore_onto_an_earlier_release() {
        let backup = backup(BackupMethod::Logical, "26.1.0", 17);

        let refused = backup
            .restorable_onto(&target("26.0.0", 17))
            .expect_err("an archive from a later release was accepted");

        assert!(
            matches!(refused, CoreError::BackupFromALaterRelease { .. }),
            "got {refused}"
        );
    }

    #[test]
    fn a_physical_archive_does_not_cross_a_postgres_major() {
        let backup = backup(BackupMethod::Physical, "26.0.0", 17);

        let refused = backup
            .restorable_onto(&target("26.0.0", 18))
            .expect_err("a base backup was accepted onto another engine");

        assert!(
            matches!(refused, CoreError::BackupLockedToPostgresMajor { .. }),
            "got {refused}"
        );
    }

    #[test]
    fn a_logical_archive_crosses_a_postgres_major() {
        let backup = backup(BackupMethod::Logical, "26.0.0", 17);

        assert!(
            backup.restorable_onto(&target("26.0.0", 18)).is_ok(),
            "surviving a major upgrade is the whole reason to keep logical dumps"
        );
    }

    #[test]
    fn an_archive_does_not_restore_onto_another_product() {
        let mut backup = backup(BackupMethod::Logical, "26.0.0", 17);
        backup.release =
            ReleaseId::new(DeploymentKind::Ferriskey, Version::parse("26.0.0").unwrap());

        assert!(backup.restorable_onto(&target("26.0.0", 17)).is_err());
    }

    #[test]
    fn an_archive_knows_how_long_it_took() {
        let backup = backup(BackupMethod::Physical, "26.0.0", 17);

        assert_eq!(backup.duration().num_minutes(), 30);
    }
}
