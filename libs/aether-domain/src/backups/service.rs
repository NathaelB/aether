//! Recording what a data plane says about an archive.
//!
//! Two entry points and they are not symmetrical, which is the whole design.
//! An archive that exists becomes a row. An archive that does not exist
//! becomes an audit entry and nothing else, because there is no such thing as
//! a backup that failed -- there is an attempt that was made, and the platform
//! keeps a record of attempts somewhere it already keeps records of work.

use std::num::NonZeroU64;

use aether_auth::Identity;
use tracing::{info, warn};

use crate::{
    CoreError, generate_uuid_v7,
    audit::{
        AuditAction, AuditEntry, AuditEntryId, AuditActor, AuditTarget, AuditTargetKind,
        ports::AuditRepository,
    },
    backups::{
        ArchivePrefix, Backup, BackupId,
        commands::{RecordArchiveCommand, RecordArchiveFailureCommand},
        ports::BackupRepository,
    },
    catalog::ReleaseId,
    deployments::ports::DeploymentRepository,
};

/// The action name an attempt is recorded under, in the namespaced form
/// `AuditAction` already uses elsewhere.
const ARCHIVE_FAILED: &str = "deployment.backup.failed";

/// Only Herald speaks for a data plane.
///
/// The same rule claim, ack, heartbeat and outcome already apply. A caller able
/// to forge an archive report could record an archive that does not exist, and
/// the platform would offer it as a restore.
fn only_herald(identity: &Identity) -> Result<(), CoreError> {
    if identity.username().contains("herald-service") {
        return Ok(());
    }

    Err(CoreError::PermissionDenied {
        reason: "only herald can report an archive".to_string(),
    })
}

pub struct BackupServiceImpl<B, D, A>
where
    B: BackupRepository,
    D: DeploymentRepository,
    A: AuditRepository,
{
    backups: B,
    deployments: D,
    audit: A,
}

impl<B, D, A> BackupServiceImpl<B, D, A>
where
    B: BackupRepository,
    D: DeploymentRepository,
    A: AuditRepository,
{
    pub fn new(backups: B, deployments: D, audit: A) -> Self {
        Self {
            backups,
            deployments,
            audit,
        }
    }

    /// Records an archive a data plane reported.
    ///
    /// Idempotent on the object key, because reports are at-least-once by
    /// design: Herald resends what it could not confirm, and an archive
    /// recorded twice would be counted twice by retention and offered twice as
    /// a restore. The second report returns the first archive unchanged rather
    /// than failing, so a redelivery is not an error anybody has to handle.
    pub async fn record_archive(
        &self,
        identity: Identity,
        command: RecordArchiveCommand,
    ) -> Result<Backup, CoreError> {
        only_herald(&identity)?;

        let deployment = self
            .deployments
            .get_by_id(command.deployment_id)
            .await?
            .ok_or(CoreError::DeploymentNotFound {
                id: command.deployment_id.0,
            })?;

        // A data plane may only report about what runs on it. Without this a
        // misconfigured Herald could record archives against another data
        // plane's deployments, and a restore would later be offered an archive
        // that is not in a bucket it can reach.
        if deployment.dataplane_id != command.dataplane_id {
            return Err(CoreError::PermissionDenied {
                reason: "this deployment does not run on that data plane".to_string(),
            });
        }

        if let Some(existing) = self
            .backups
            .find_by_object_key(&command.deployment_id, &command.object_key)
            .await?
        {
            info!(
                backup = %existing.id,
                deployment = %command.deployment_id,
                "this archive was already recorded: the report is a redelivery"
            );
            return Ok(existing);
        }

        // Refused rather than stored. The type says the same thing with
        // NonZeroU64 and the column says it with a CHECK; this is where a
        // report gets a message naming what was wrong with it.
        let size_bytes = NonZeroU64::new(command.size_bytes).ok_or_else(|| {
            CoreError::InvalidArchiveReport {
                deployment: command.deployment_id.0,
                reason: "an archive of zero bytes is a backup that did not happen".to_string(),
            }
        })?;

        if command.finished_at < command.started_at {
            return Err(CoreError::InvalidArchiveReport {
                deployment: command.deployment_id.0,
                reason: "it finished before it started".to_string(),
            });
        }

        // Built here rather than taken from the report. The prefix is derived
        // from the organisation and the deployment the control plane already
        // holds, so a report cannot address another tenant's archive by
        // claiming a path.
        let location = ArchivePrefix::new(deployment.organisation_id, deployment.id)
            .object(&command.object_key)?;

        let backup = Backup {
            id: BackupId(generate_uuid_v7()),
            deployment_id: deployment.id,
            organisation_id: deployment.organisation_id,
            // What the control plane knows was running, not what the report
            // claims. A data plane reporting a version is reporting something
            // it read off a pod, and the two disagree during an upgrade.
            release: ReleaseId::new(deployment.kind.clone(), deployment.version.clone()),
            postgres_major: command.postgres_major,
            method: command.method,
            key: command.key,
            location,
            size_bytes,
            started_at: command.started_at,
            finished_at: command.finished_at,
        };

        self.backups.record(backup.clone()).await?;

        info!(
            backup = %backup.id,
            deployment = %backup.deployment_id,
            size_bytes = backup.size_bytes.get(),
            "an archive was recorded"
        );

        Ok(backup)
    }

    /// Records that an archive was attempted and did not happen.
    ///
    /// No backup row, ever. A row saying an archive failed is a restore
    /// somebody eventually attempts; the audit trail is where the platform
    /// already keeps what was tried.
    pub async fn record_archive_failure(
        &self,
        identity: Identity,
        command: RecordArchiveFailureCommand,
    ) -> Result<(), CoreError> {
        only_herald(&identity)?;

        let deployment = self
            .deployments
            .get_by_id(command.deployment_id)
            .await?
            .ok_or(CoreError::DeploymentNotFound {
                id: command.deployment_id.0,
            })?;

        if deployment.dataplane_id != command.dataplane_id {
            return Err(CoreError::PermissionDenied {
                reason: "this deployment does not run on that data plane".to_string(),
            });
        }

        warn!(
            deployment = %deployment.id,
            reason = %command.reason,
            "an archive was attempted and did not happen"
        );

        self.audit
            .append(AuditEntry::record(
                AuditEntryId(generate_uuid_v7()),
                deployment.organisation_id,
                // The data plane acted, and nobody asked it to: this is a
                // schedule firing, not a person clicking.
                AuditActor::System,
                AuditAction(ARCHIVE_FAILED.to_string()),
                AuditTarget {
                    kind: AuditTargetKind::Deployment,
                    id: deployment.id.0,
                },
                None,
                command.attempted_at,
            ))
            .await
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};
    use uuid::Uuid;

    use super::*;
    use crate::{
        audit::ports::MockAuditRepository,
        backups::{
            BackupMethod, PostgresMajor,
            backup::fixtures::backup,
            keys::{KeyName, KeyRef, KeyVersion, ProviderName},
            ports::MockBackupRepository,
        },
        dataplane::value_objects::{DataPlaneId, DeploymentResources},
        deployments::{
            Deployment, DeploymentId, DeploymentKind, DeploymentName, DeploymentStatus,
            network::NetworkAccess, ports::MockDeploymentRepository,
        },
        organisation::OrganisationId,
        upgrades::policy::AutoUpgradePolicy,
        user::UserId,
        version::Version,
    };

    fn deployment_id() -> DeploymentId {
        DeploymentId(Uuid::from_u128(2))
    }

    fn a_deployment() -> Deployment {
        let now = Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap();

        Deployment {
            id: deployment_id(),
            organisation_id: OrganisationId(Uuid::from_u128(1)),
            dataplane_id: DataPlaneId(Uuid::from_u128(9)),
            name: DeploymentName("archived".to_string()),
            kind: DeploymentKind::Keycloak,
            version: Version::parse("26.0.0").unwrap(),
            status: DeploymentStatus::Successful,
            namespace: "tenant".to_string(),
            resources: DeploymentResources::DEFAULT,
            created_by: UserId(Uuid::from_u128(3)),
            created_at: now,
            updated_at: now,
            deployed_at: None,
            deleted_at: None,
            auto_upgrade: AutoUpgradePolicy::Manual,
            maintenance_window: None,
            network_access: NetworkAccess::Open,
        }
    }

    fn herald() -> Identity {
        Identity::Client(aether_auth::Client {
            id: "id".to_string(),
            client_id: "herald-service".to_string(),
            roles: vec![],
            scopes: vec![],
        })
    }

    fn a_report() -> RecordArchiveCommand {
        let finished_at = Utc.with_ymd_and_hms(2026, 9, 12, 2, 30, 0).unwrap();

        RecordArchiveCommand {
            dataplane_id: DataPlaneId(Uuid::from_u128(9)),
            deployment_id: deployment_id(),
            object_key: "base/20260912T0230Z/data.tar.gz".to_string(),
            method: BackupMethod::Physical,
            postgres_major: PostgresMajor(17),
            key: KeyRef::new(
                ProviderName::platform(),
                KeyName::new("aether-backups").unwrap(),
                KeyVersion::new(1),
            ),
            size_bytes: 4_136_598,
            started_at: finished_at - Duration::minutes(5),
            finished_at,
        }
    }

    fn deployments_returning_one() -> MockDeploymentRepository {
        let mut deployments = MockDeploymentRepository::new();
        deployments
            .expect_get_by_id()
            .returning(|_| Box::pin(async { Ok(Some(a_deployment())) }));
        deployments
    }

    #[tokio::test]
    async fn an_archive_is_recorded_against_what_the_control_plane_knows_was_running() {
        let mut backups = MockBackupRepository::new();
        backups
            .expect_find_by_object_key()
            .returning(|_, _| Box::pin(async { Ok(None) }));
        backups
            .expect_record()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));

        let service =
            BackupServiceImpl::new(backups, deployments_returning_one(), MockAuditRepository::new());

        let recorded = service.record_archive(herald(), a_report()).await.expect("recorded");

        // The release comes from the deployment, not from the report. A data
        // plane reporting a version is reporting what it read off a pod, and
        // the two disagree for the length of an upgrade.
        assert_eq!(recorded.release.version, Version::parse("26.0.0").unwrap());
        assert_eq!(recorded.release.kind, DeploymentKind::Keycloak);
        assert_eq!(recorded.size_bytes.get(), 4_136_598);
        // Addressed through the prefix the control plane builds, so a report
        // cannot name another tenant's archive by claiming a path.
        assert_eq!(
            recorded.location.as_path(),
            format!(
                "{}/{}/base/20260912T0230Z/data.tar.gz",
                Uuid::from_u128(1),
                Uuid::from_u128(2)
            )
        );
    }

    /// Reports are at-least-once by design. An archive recorded twice would be
    /// counted twice by retention and offered twice as a restore.
    #[tokio::test]
    async fn reporting_the_same_archive_again_records_it_once() {
        let existing = backup(BackupMethod::Physical, "26.0.0", 17);
        let existing_id = existing.id;

        let mut backups = MockBackupRepository::new();
        backups
            .expect_find_by_object_key()
            .returning(move |_, _| {
                let existing = existing.clone();
                Box::pin(async move { Ok(Some(existing)) })
            });
        // The assertion that matters: nothing is written on the second report.
        backups.expect_record().never();

        let service =
            BackupServiceImpl::new(backups, deployments_returning_one(), MockAuditRepository::new());

        let returned = service.record_archive(herald(), a_report()).await.expect("recorded");

        assert_eq!(returned.id, existing_id);
    }

    #[tokio::test]
    async fn an_archive_of_nothing_is_refused_with_a_reason() {
        let mut backups = MockBackupRepository::new();
        backups
            .expect_find_by_object_key()
            .returning(|_, _| Box::pin(async { Ok(None) }));
        backups.expect_record().never();

        let service =
            BackupServiceImpl::new(backups, deployments_returning_one(), MockAuditRepository::new());

        let refused = service
            .record_archive(herald(), RecordArchiveCommand {
                size_bytes: 0,
                ..a_report()
            })
            .await
            .expect_err("an archive of zero bytes was recorded");

        assert!(matches!(refused, CoreError::InvalidArchiveReport { .. }));
    }

    #[tokio::test]
    async fn an_archive_that_finished_before_it_started_is_refused() {
        let mut backups = MockBackupRepository::new();
        backups
            .expect_find_by_object_key()
            .returning(|_, _| Box::pin(async { Ok(None) }));
        backups.expect_record().never();

        let report = a_report();
        let service =
            BackupServiceImpl::new(backups, deployments_returning_one(), MockAuditRepository::new());

        let refused = service
            .record_archive(herald(), RecordArchiveCommand {
                finished_at: report.started_at - Duration::seconds(1),
                ..report
            })
            .await
            .expect_err("an archive that ran backwards was recorded");

        assert!(matches!(refused, CoreError::InvalidArchiveReport { .. }));
    }

    /// A key that climbs out of the prefix is refused before anything is
    /// written, which is the last place a report could address another tenant.
    #[tokio::test]
    async fn a_report_cannot_address_another_tenants_archive() {
        let mut backups = MockBackupRepository::new();
        backups
            .expect_find_by_object_key()
            .returning(|_, _| Box::pin(async { Ok(None) }));
        backups.expect_record().never();

        let service =
            BackupServiceImpl::new(backups, deployments_returning_one(), MockAuditRepository::new());

        let refused = service
            .record_archive(herald(), RecordArchiveCommand {
                object_key: "../../someone-else/base/data.tar.gz".to_string(),
                ..a_report()
            })
            .await
            .expect_err("a report addressed another prefix");

        assert!(matches!(refused, CoreError::ObjectStore(_)), "got {refused}");
    }

    /// The same rule claim, ack, heartbeat and outcome already apply. A caller
    /// able to forge a report could record an archive that does not exist, and
    /// the platform would offer it as a restore.
    #[tokio::test]
    async fn only_herald_may_report_an_archive() {
        let mut backups = MockBackupRepository::new();
        backups.expect_record().never();
        backups.expect_find_by_object_key().never();

        let anybody = Identity::Client(aether_auth::Client {
            id: "id".to_string(),
            client_id: "console".to_string(),
            roles: vec![],
            scopes: vec![],
        });

        let service = BackupServiceImpl::new(
            backups,
            MockDeploymentRepository::new(),
            MockAuditRepository::new(),
        );

        let refused = service
            .record_archive(anybody, a_report())
            .await
            .expect_err("anybody could report an archive");

        assert!(matches!(refused, CoreError::PermissionDenied { .. }));
    }

    /// A data plane may only report about what runs on it. Otherwise a
    /// misconfigured Herald records archives against another cluster's
    /// deployments, and a restore is later offered an archive that is not in a
    /// bucket it can reach.
    #[tokio::test]
    async fn a_data_plane_cannot_report_another_ones_archive() {
        let mut backups = MockBackupRepository::new();
        backups.expect_record().never();
        backups.expect_find_by_object_key().never();

        let service =
            BackupServiceImpl::new(backups, deployments_returning_one(), MockAuditRepository::new());

        let refused = service
            .record_archive(
                herald(),
                RecordArchiveCommand {
                    dataplane_id: DataPlaneId(Uuid::from_u128(404)),
                    ..a_report()
                },
            )
            .await
            .expect_err("a data plane reported another one's archive");

        assert!(matches!(refused, CoreError::PermissionDenied { .. }));
    }

    /// The rule the whole module is shaped around: an attempt that produced no
    /// archive produces no row.
    #[tokio::test]
    async fn a_failed_attempt_is_audited_and_writes_no_backup() {
        let mut backups = MockBackupRepository::new();
        backups.expect_record().never();
        backups.expect_find_by_object_key().never();

        let mut audit = MockAuditRepository::new();
        audit
            .expect_append()
            .times(1)
            .returning(|entry| {
                assert_eq!(entry.action.0, ARCHIVE_FAILED);
                assert_eq!(entry.target.id, Uuid::from_u128(2));
                Box::pin(async { Ok(()) })
            });

        let service = BackupServiceImpl::new(backups, deployments_returning_one(), audit);

        service
            .record_archive_failure(herald(), RecordArchiveFailureCommand {
                dataplane_id: DataPlaneId(Uuid::from_u128(9)),
                deployment_id: deployment_id(),
                reason: "cannot proceed with the backup as the cluster has no backup section"
                    .to_string(),
                attempted_at: Utc::now(),
            })
            .await
            .expect("the attempt is recorded");
    }
}
