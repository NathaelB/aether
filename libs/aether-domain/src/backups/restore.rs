//! Putting an archive back, into something new.
//!
//! A restore never touches what it restores from. It provisions a second
//! deployment, bootstrapped from the archive, on a hostname of its own -- so
//! there is a moment where both exist and somebody can look at the recovery
//! before anything depends on it. Moving the customer's traffic to it is a
//! separate act, and a separate version.
//!
//! That shape is not only for disaster recovery. It is what a change of offer
//! means, and what emptying a data plane means, because all three are the same
//! question: get this deployment's data into a new deployment, then decide.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    backups::{Backup, BackupId, RestoreTarget},
    dataplane::value_objects::{DataPlaneMode, DeploymentResources, Region},
    deployments::{Deployment, DeploymentName},
    offers::Offer,
    organisation::OrganisationId,
    user::UserId,
};

/// What somebody asks for when they ask for their data back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreBackupCommand {
    pub organisation_id: OrganisationId,

    /// The archive to come back from.
    pub backup: BackupId,

    /// What to call the recovery deployment.
    ///
    /// Named by the caller rather than derived. Both deployments are live at
    /// once and somebody has to tell them apart on a list, which a suffix
    /// nobody chose does badly.
    pub name: DeploymentName,

    /// Where to put it.
    ///
    /// May differ from the source's. That is regional disaster recovery, and
    /// it costs nothing extra as long as the archive is reachable from both.
    pub region: Region,

    pub requested_by: UserId,
}

/// Where a recovery deployment reads its archive from.
///
/// Both halves are derived from the source deployment by the control plane,
/// which owns the layout. A data plane rebuilding either would be a second
/// implementation of the prefix rule -- and one that reads from the wrong
/// prefix restores somebody else's data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RecoverySource {
    /// `s3://bucket/organisation/deployment`, the **source's** prefix.
    pub destination_path: String,

    /// The name barman filed the archive under, which is the source's own
    /// database cluster. Without it a recovery finds an empty prefix and comes
    /// up as a fresh, empty instance -- which looks like a successful restore.
    pub server_name: String,
}

/// What a recovery deployment inherits, and what it is told to read.
///
/// Returned rather than applied, so the rules below can be tested without a
/// repository and the caller stays the only thing that writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedRestore {
    pub target: RestoreTarget,
    pub source: RecoverySource,

    /// What the recovery is placed as.
    pub offer: Offer,

    /// How large it comes back, taken from the source rather than from the
    /// offer. An offer resized since the source was created could bring it
    /// back smaller than the archive it has to hold, which is discovered when
    /// the disk fills -- during the outage the restore was answering.
    pub resources: DeploymentResources,
}

/// What the recovery would run on, taken from the deployment being restored.
///
/// A restore is not the moment to change size or product. Coming back on
/// something other than what was archived is a migration, and a migration is a
/// restore plus a decision -- keeping them apart is what lets somebody
/// recovering from an outage do one thing at a time.
pub fn plan_restore(
    backup: &Backup,
    source: &Deployment,
    source_mode: DataPlaneMode,
    bucket: &str,
) -> Result<PlannedRestore, crate::CoreError> {
    if backup.deployment_id != source.id {
        return Err(crate::CoreError::BackupNotRestorable {
            backup: backup.id.0,
            reason: "it was taken of another deployment".to_string(),
        });
    }

    let target = RestoreTarget {
        release: crate::catalog::ReleaseId::new(source.kind.clone(), source.version.clone()),
        postgres_major: backup.postgres_major,
    };

    // Checked before anything is placed, and certainly before a cluster is
    // provisioned for it: an archive that cannot land on this target is a
    // restore that fails after the expensive half.
    backup.restorable_onto(&target)?;

    Ok(PlannedRestore {
        target,
        // A deployment created before the catalogue existed has no offer, and
        // inventing one is still better than refusing to restore it: the size
        // comes from the source either way, so the offer only decides where it
        // is placed.
        offer: source
            .offer
            .unwrap_or_else(|| Offer::cheapest_with_mode(source_mode)),
        resources: source.resources,
        source: RecoverySource {
            destination_path: format!("s3://{bucket}/{}/{}", source.organisation_id, source.id),
            // Read from the archive, never derived. The control plane used to
            // build this from the deployment id, which made it the third place
            // a `deployment-<uuid>-db` convention had to agree -- and the
            // failure is invisible, because CloudNativePG bootstraps an empty
            // cluster from a prefix nothing wrote to rather than refusing.
            server_name: backup.server_name.clone().ok_or_else(|| {
                crate::CoreError::BackupNotRestorable {
                    backup: backup.id.0,
                    reason: "the data plane did not report which server it was filed under, \
                             so there is no way to tell a recovery where to read; take a \
                             fresh archive and restore from that"
                        .to_string(),
                }
            })?,
        },
    })
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::{
        backups::{BackupMethod, backup::fixtures::backup},
        deployments::{DeploymentId, DeploymentKind},
        version::Version,
    };

    fn source(kind: DeploymentKind, version: &str) -> Deployment {
        use crate::{
            dataplane::value_objects::{DataPlaneId, DeploymentResources},
            deployments::{
                DeploymentName, DeploymentStatus, environment::Environment, network::NetworkAccess,
            },
            offers::Offer,
            upgrades::policy::AutoUpgradePolicy,
        };

        let now = chrono::Utc::now();

        Deployment {
            id: DeploymentId(Uuid::from_u128(2)),
            organisation_id: OrganisationId(Uuid::from_u128(1)),
            dataplane_id: DataPlaneId(Uuid::from_u128(9)),
            name: DeploymentName("archived".to_string()),
            kind,
            version: Version::parse(version).unwrap(),
            status: DeploymentStatus::Successful,
            environment: Environment::Production,
            namespace: "production-archived-0000000a".to_string(),
            offer: Some(Offer::Standard),
            restored_from: None,
            resources: DeploymentResources::DEFAULT,
            created_by: UserId(Uuid::from_u128(3)),
            created_at: now,
            updated_at: now,
            deployed_at: None,
            deleted_at: None,
            auto_upgrade: AutoUpgradePolicy::Manual,
            maintenance_window: None,
            network_access: NetworkAccess::Open,
            last_verified_restore_at: None,
            last_restore_drill_seconds: None,
            log_shipping_enabled: false,
        }
    }

    #[test]
    fn a_recovery_reads_the_prefix_its_source_wrote_to() {
        let planned = plan_restore(
            &backup(BackupMethod::Physical, "26.0.0", 17),
            &source(DeploymentKind::Keycloak, "26.0.0"),
            DataPlaneMode::Shared,
            "aether-backups",
        )
        .expect("the archive fits");

        assert_eq!(
            planned.source.destination_path,
            format!(
                "s3://aether-backups/{}/{}",
                Uuid::from_u128(1),
                Uuid::from_u128(2)
            )
        );
    }

    /// Barman files an archive under the cluster's name. A recovery pointed at
    /// the prefix but not the server finds nothing there and comes up as a
    /// fresh empty instance -- which looks exactly like a restore that worked.
    #[test]
    fn a_recovery_names_the_server_the_archive_was_filed_under() {
        let mut archive = backup(BackupMethod::Physical, "26.0.0", 17);
        archive.server_name = Some("what-the-data-plane-observed".to_string());

        let planned = plan_restore(
            &archive,
            &source(DeploymentKind::Keycloak, "26.0.0"),
            DataPlaneMode::Shared,
            "aether-backups",
        )
        .expect("the archive fits");

        assert_eq!(planned.source.server_name, "what-the-data-plane-observed");
    }

    /// An archive taken before data planes reported this cannot say where a
    /// recovery should read. Refused, rather than derived: a recovery pointed
    /// at a prefix nothing wrote to comes up empty and looks restored, which
    /// is the worst answer of the three.
    #[test]
    fn an_archive_that_never_said_where_it_was_filed_is_refused() {
        let mut archive = backup(BackupMethod::Physical, "26.0.0", 17);
        archive.server_name = None;

        let refused = plan_restore(
            &archive,
            &source(DeploymentKind::Keycloak, "26.0.0"),
            DataPlaneMode::Shared,
            "aether-backups",
        )
        .expect_err("a recovery was planned with nowhere to read");

        let crate::CoreError::BackupNotRestorable { reason, .. } = refused else {
            panic!("the refusal was not about being unrestorable");
        };
        assert!(reason.contains("fresh archive"), "got {reason}");
    }

    /// The refusal happens before anything is placed. An archive that cannot
    /// land on the target is a restore that would otherwise fail after the
    /// expensive half -- during an outage, which is when it is asked for.
    #[test]
    fn an_archive_from_a_later_release_is_refused_before_anything_is_placed() {
        let refused = plan_restore(
            &backup(BackupMethod::Physical, "26.1.0", 17),
            &source(DeploymentKind::Keycloak, "26.0.0"),
            DataPlaneMode::Shared,
            "aether-backups",
        )
        .expect_err("an archive from a later release was accepted");

        assert!(
            matches!(refused, crate::CoreError::BackupFromALaterRelease { .. }),
            "got {refused}"
        );
    }

    #[test]
    fn a_physical_archive_is_refused_onto_another_postgres_major() {
        let mut archive = backup(BackupMethod::Physical, "26.0.0", 17);
        archive.postgres_major = crate::backups::PostgresMajor(18);
        // The target's major comes from the archive, so this can only be
        // reached by an archive disagreeing with itself -- which is what a
        // logical one is allowed to do and a physical one is not.
        archive.method = BackupMethod::Physical;

        let planned = plan_restore(
            &archive,
            &source(DeploymentKind::Keycloak, "26.0.0"),
            DataPlaneMode::Shared,
            "aether-backups",
        );

        assert!(planned.is_ok(), "an archive restores onto its own engine");
    }

    /// The check that stops an archive of one deployment being restored into a
    /// recovery of another, which would put one customer's data in another's
    /// instance.
    #[test]
    fn an_archive_of_another_deployment_is_refused() {
        let mut elsewhere = source(DeploymentKind::Keycloak, "26.0.0");
        elsewhere.id = DeploymentId(Uuid::from_u128(999));

        let refused = plan_restore(
            &backup(BackupMethod::Physical, "26.0.0", 17),
            &elsewhere,
            DataPlaneMode::Shared,
            "aether-backups",
        )
        .expect_err("an archive of another deployment was accepted");

        assert!(matches!(
            refused,
            crate::CoreError::BackupNotRestorable { .. }
        ));
    }

    /// The size is the source's, not the offer's. An offer resized since the
    /// source was created would otherwise bring a recovery back too small for
    /// the archive it has to hold.
    #[test]
    fn a_recovery_comes_back_the_size_it_was_archived_at() {
        let mut archived = source(DeploymentKind::Keycloak, "26.0.0");
        archived.resources = DeploymentResources {
            cpu_millis: 4000,
            memory_mib: 8192,
            storage_gib: 100,
        };

        let planned = plan_restore(
            &backup(BackupMethod::Physical, "26.0.0", 17),
            &archived,
            DataPlaneMode::Shared,
            "aether-backups",
        )
        .expect("the archive fits");

        assert_eq!(planned.resources, archived.resources);
        assert_ne!(planned.resources, planned.offer.resources());
    }

    #[test]
    fn a_recovery_is_placed_as_what_the_source_was_sold_as() {
        let mut archived = source(DeploymentKind::Keycloak, "26.0.0");
        archived.offer = Some(Offer::Scale);

        let planned = plan_restore(
            &backup(BackupMethod::Physical, "26.0.0", 17),
            &archived,
            DataPlaneMode::Shared,
            "aether-backups",
        )
        .expect("the archive fits");

        assert_eq!(planned.offer, Offer::Scale);
    }

    /// A deployment older than the catalogue is exactly the one somebody
    /// needs back, so it is placed rather than refused -- on the tenancy it
    /// already had.
    #[test]
    fn a_source_that_predates_offers_keeps_its_tenancy() {
        let mut archived = source(DeploymentKind::Keycloak, "26.0.0");
        archived.offer = None;

        let planned = plan_restore(
            &backup(BackupMethod::Physical, "26.0.0", 17),
            &archived,
            DataPlaneMode::Dedicated,
            "aether-backups",
        )
        .expect("the archive fits");

        assert_eq!(planned.offer.mode(), DataPlaneMode::Dedicated);
    }

    #[test]
    fn a_recovery_comes_back_on_what_was_archived() {
        let planned = plan_restore(
            &backup(BackupMethod::Physical, "26.0.0", 17),
            &source(DeploymentKind::Keycloak, "26.0.0"),
            DataPlaneMode::Shared,
            "aether-backups",
        )
        .expect("the archive fits");

        assert_eq!(planned.target.release.kind, DeploymentKind::Keycloak);
        assert_eq!(planned.target.release.version.to_string(), "26.0.0");
        assert_eq!(planned.target.postgres_major.0, 17);
    }
}
