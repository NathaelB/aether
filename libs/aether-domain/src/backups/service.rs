//! Recording what a data plane says about an archive.
//!
//! Two entry points and they are not symmetrical, which is the whole design.
//! An archive that exists becomes a row. An archive that does not exist
//! becomes an audit entry and nothing else, because there is no such thing as
//! a backup that failed -- there is an attempt that was made, and the platform
//! keeps a record of attempts somewhere it already keeps records of work.

use std::num::NonZeroU64;

use aether_auth::Identity;
use chrono::Utc;
use tracing::{info, warn};

use crate::{
    CoreError,
    audit::{
        AuditAction, AuditActor, AuditEntry, AuditEntryId, AuditTarget, AuditTargetKind,
        ports::AuditRepository,
    },
    backups::{
        ArchivePrefix, Backup, BackupId, BackupMethod, BackupSchedule,
        commands::{
            AskForBackupCommand, RecordArchiveCommand, RecordArchiveFailureCommand,
            SetBackupScheduleCommand,
        },
        ports::{BackupPolicy, BackupRepository, BackupScheduleRepository},
    },
    catalog::ReleaseId,
    deployments::{Deployment, DeploymentId, ports::DeploymentRepository},
    generate_uuid_v7,
    organisation::OrganisationId,
    platform::{PlatformRight, ports::PlatformPolicy},
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

pub struct BackupServiceImpl<B, S, D, A, P, PP>
where
    B: BackupRepository,
    S: BackupScheduleRepository,
    D: DeploymentRepository,
    A: AuditRepository,
    P: BackupPolicy,
    PP: PlatformPolicy,
{
    backups: B,
    schedules: S,
    deployments: D,
    audit: A,
    policy: P,

    /// The other way in. Archiving somebody's deployment is something they do
    /// to their own and something an operator does to anybody's, and those are
    /// two different rights rather than one rule with an exception.
    platform: PP,
}

impl<B, S, D, A, P, PP> BackupServiceImpl<B, S, D, A, P, PP>
where
    B: BackupRepository,
    S: BackupScheduleRepository,
    D: DeploymentRepository,
    A: AuditRepository,
    P: BackupPolicy,
    PP: PlatformPolicy,
{
    pub fn new(
        backups: B,
        schedules: S,
        deployments: D,
        audit: A,
        policy: P,
        platform: PP,
    ) -> Self {
        Self {
            backups,
            schedules,
            deployments,
            audit,
            policy,
            platform,
        }
    }

    /// Whether this caller may take an archive of this deployment.
    ///
    /// Two ways in and one rule: an operator holding `act_on_tenant` on
    /// anybody's deployment, or a member holding `manage_backups` on their
    /// own. A second endpoint for the operator path would be the same code
    /// behind a different door, and the two would drift the day one of them
    /// gained a check.
    ///
    /// The platform right is tried first and its refusal is discarded: most
    /// callers here are customers, and an operator's refusal is not the answer
    /// a customer should be given.
    async fn may_archive(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> Result<(), CoreError> {
        if self
            .platform
            .require(identity.clone(), PlatformRight::ActOnTenant)
            .await
            .is_ok()
        {
            return Ok(());
        }

        self.policy
            .can_manage_backups(identity, organisation_id)
            .await
    }

    /// Records that somebody asked for an archive, refusing a second while the
    /// first is still coming.
    ///
    /// Returns the deployment, because the caller has to tell the data plane
    /// where to write and needs the same row this already read to authorise
    /// the request.
    pub async fn ask_for_backup(
        &self,
        identity: Identity,
        command: AskForBackupCommand,
    ) -> Result<Deployment, CoreError> {
        self.may_archive(identity, command.organisation_id).await?;

        // Both halves, as everywhere else here: without the first, somebody
        // outside the organisation archives it; without the second, a member
        // of one organisation archives another's by pairing their own id with
        // a deployment id they guessed.
        let deployment = self
            .deployment_in(command.organisation_id, command.deployment_id)
            .await?;

        Ok(deployment)
    }

    /// The deployment, once it is established that it is the one being asked
    /// about.
    ///
    /// Both halves matter. Without the first, somebody outside the
    /// organisation reads its archives; without the second, a member of one
    /// organisation reads another's by pairing their own organisation with a
    /// deployment id they guessed.
    async fn deployment_in(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<Deployment, CoreError> {
        let deployment = self
            .deployments
            .get_by_id(deployment_id)
            .await?
            .filter(|deployment| deployment.organisation_id == organisation_id)
            .ok_or(CoreError::DeploymentNotFound {
                id: deployment_id.0,
            })?;

        Ok(deployment)
    }

    /// The archive and the deployment it was taken of, once it is established
    /// that the caller may restore it.
    ///
    /// Restoring is `can_manage_backups` rather than `can_view_backups`: it
    /// provisions an instance and it copies data out of an archive, neither of
    /// which is a read.
    ///
    /// The archive is matched against the organisation, not only against the
    /// deployment. Without that, somebody restores an archive belonging to
    /// another tenant by naming an id they guessed, and the resulting instance
    /// is theirs.
    pub async fn restorable(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        backup: BackupId,
    ) -> Result<(Backup, Deployment), CoreError> {
        self.policy
            .can_manage_backups(identity, organisation_id)
            .await?;

        let archive = self
            .backups
            .get(&backup)
            .await?
            .filter(|archive| archive.organisation_id == organisation_id)
            .ok_or(CoreError::BackupNotFound { id: backup.0 })?;

        let source = self
            .deployment_in(organisation_id, archive.deployment_id)
            .await?;

        Ok((archive, source))
    }

    /// One deployment's archives, newest first.
    pub async fn list_backups(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<Vec<Backup>, CoreError> {
        // Before the read, not after. A refusal that first fetched the rows
        // has already done the thing it is refusing.
        self.policy
            .can_view_backups(identity, organisation_id)
            .await?;

        self.deployment_in(organisation_id, deployment_id).await?;

        self.backups.list_for_deployment(&deployment_id).await
    }

    /// When this deployment is archived, and how much history is kept.
    pub async fn get_backup_schedule(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<BackupSchedule, CoreError> {
        self.policy
            .can_view_backups(identity, organisation_id)
            .await?;

        let deployment = self.deployment_in(organisation_id, deployment_id).await?;

        // The default rather than nothing. A deployment created before the
        // platform wrote schedules is archived on the same terms as one
        // created after it, and answering "none" would describe the row
        // instead of the deployment.
        Ok(self
            .schedules
            .get(&deployment_id)
            .await?
            .unwrap_or_else(|| {
                BackupSchedule::default_for(deployment.id, deployment.organisation_id, Utc::now())
            }))
    }

    /// Changes when this deployment is archived.
    ///
    /// Returns what was stored, not what was asked for. The two are the same
    /// today and the caller should not have to know that: a screen drawing the
    /// request rather than the row is a screen that shows a change nobody
    /// made.
    pub async fn set_backup_schedule(
        &self,
        identity: Identity,
        command: SetBackupScheduleCommand,
    ) -> Result<BackupSchedule, CoreError> {
        self.policy
            .can_manage_backups(identity, command.organisation_id)
            .await?;

        let deployment = self
            .deployment_in(command.organisation_id, command.deployment_id)
            .await?;

        let now = Utc::now();
        let existing = self.schedules.get(&command.deployment_id).await?;

        let schedule = BackupSchedule {
            deployment_id: deployment.id,
            organisation_id: deployment.organisation_id,
            cadence: command.cadence,
            zone: command.zone,
            retention: command.retention,
            // Not settable here. The only mechanism a data plane carries out
            // is a base backup to the object store, and offering the other one
            // on this call would accept a choice nothing can honour.
            method: existing
                .as_ref()
                .map(|schedule| schedule.method)
                .unwrap_or(BackupMethod::Physical),
            enabled: command.enabled,
            // Kept from the row it replaces. When a deployment was first
            // scheduled is a fact about the deployment, and rewriting it on
            // every edit would lose it.
            created_at: existing.map(|schedule| schedule.created_at).unwrap_or(now),
            updated_at: now,
        };

        self.schedules.save(schedule.clone()).await?;

        info!(
            deployment = %schedule.deployment_id,
            enabled = schedule.enabled,
            "the archive schedule was changed"
        );

        Ok(schedule)
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
        let size_bytes =
            NonZeroU64::new(command.size_bytes).ok_or_else(|| CoreError::InvalidArchiveReport {
                deployment: command.deployment_id.0,
                reason: "an archive of zero bytes is a backup that did not happen".to_string(),
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
            protection: command.protection,
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
    use crate::platform::{PlatformRight, fixtures::Granting};
    use crate::{
        audit::ports::MockAuditRepository,
        backups::{
            ArchiveProtection, BackupMethod, Cadence, PostgresMajor, Retention,
            backup::fixtures::backup,
            ports::{MockBackupRepository, MockBackupScheduleRepository},
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

    /// Allows everything. The rules these tests state are about what a report
    /// may say, not about who may ask -- those have their own tests below.
    struct Permissive;

    impl BackupPolicy for Permissive {
        async fn can_view_backups(&self, _: Identity, _: OrganisationId) -> Result<(), CoreError> {
            Ok(())
        }

        async fn can_manage_backups(
            &self,
            _: Identity,
            _: OrganisationId,
        ) -> Result<(), CoreError> {
            Ok(())
        }
    }

    struct Refuses;

    impl BackupPolicy for Refuses {
        async fn can_view_backups(&self, _: Identity, _: OrganisationId) -> Result<(), CoreError> {
            Err(CoreError::PermissionDenied {
                reason: "insufficient permissions".to_string(),
            })
        }

        async fn can_manage_backups(
            &self,
            _: Identity,
            _: OrganisationId,
        ) -> Result<(), CoreError> {
            Err(CoreError::PermissionDenied {
                reason: "insufficient permissions".to_string(),
            })
        }
    }

    fn service(
        backups: MockBackupRepository,
        deployments: MockDeploymentRepository,
        audit: MockAuditRepository,
    ) -> BackupServiceImpl<
        MockBackupRepository,
        MockBackupScheduleRepository,
        MockDeploymentRepository,
        MockAuditRepository,
        Permissive,
        Granting,
    > {
        BackupServiceImpl::new(
            backups,
            MockBackupScheduleRepository::new(),
            deployments,
            audit,
            Permissive,
            // Nothing. Every test here is about a customer acting on their own
            // deployment; the operator path has its own.
            Granting::nothing(),
        )
    }

    /// Two ways in, and each one is enough on its own. Without the first
    /// test, an operator could not take a backup of a customer they are about
    /// to touch; without the second, every customer would depend on somebody
    /// at the platform to archive their own deployment.
    #[tokio::test]
    async fn an_operator_may_archive_a_deployment_that_is_not_theirs() {
        let asked = asking(Refuses, Granting::only(PlatformRight::ActOnTenant)).await;

        assert!(asked.is_ok(), "{asked:?}");
    }

    #[tokio::test]
    async fn a_member_may_archive_their_own() {
        let asked = asking(Permissive, Granting::nothing()).await;

        assert!(asked.is_ok(), "{asked:?}");
    }

    /// Neither way in. This is what the endpoint answers to somebody else's
    /// customer, and the refusal is the organisation's rather than the
    /// platform's -- most callers here are customers, and telling one they
    /// lack an operator right would send them somewhere that cannot help.
    #[tokio::test]
    async fn somebody_with_neither_is_refused() {
        let refused = asking(Refuses, Granting::nothing())
            .await
            .expect_err("a caller with neither right archived a deployment");

        assert!(
            matches!(refused, CoreError::PermissionDenied { .. }),
            "got {refused}"
        );
    }

    async fn asking<P: BackupPolicy>(
        policy: P,
        platform: Granting,
    ) -> Result<Deployment, CoreError> {
        let deployment = a_deployment();
        let organisation_id = deployment.organisation_id;
        let deployment_id = deployment.id;

        let mut deployments = MockDeploymentRepository::new();
        deployments.expect_get_by_id().returning(move |_| {
            let deployment = deployment.clone();
            Box::pin(async move { Ok(Some(deployment)) })
        });

        BackupServiceImpl::new(
            MockBackupRepository::new(),
            MockBackupScheduleRepository::new(),
            deployments,
            MockAuditRepository::new(),
            policy,
            platform,
        )
        .ask_for_backup(
            somebody(),
            AskForBackupCommand {
                organisation_id,
                deployment_id,
                requested_by: crate::user::UserId(Uuid::new_v4()),
            },
        )
        .await
    }

    fn somebody() -> Identity {
        Identity::Client(aether_auth::Client {
            id: "somebody".to_string(),
            client_id: "somebody".to_string(),
            roles: vec![],
            scopes: vec![],
        })
    }

    fn schedules_holding(existing: Option<BackupSchedule>) -> MockBackupScheduleRepository {
        let mut schedules = MockBackupScheduleRepository::new();
        schedules
            .expect_get()
            .returning(move |_| Box::pin(std::future::ready(Ok(existing.clone()))));
        schedules
            .expect_save()
            .returning(|_| Box::pin(std::future::ready(Ok(()))));

        schedules
    }

    fn reading<P: BackupPolicy>(
        backups: MockBackupRepository,
        deployments: MockDeploymentRepository,
        schedules: MockBackupScheduleRepository,
        policy: P,
    ) -> BackupServiceImpl<
        MockBackupRepository,
        MockBackupScheduleRepository,
        MockDeploymentRepository,
        MockAuditRepository,
        P,
        Granting,
    > {
        BackupServiceImpl::new(
            backups,
            schedules,
            deployments,
            MockAuditRepository::new(),
            policy,
            Granting::nothing(),
        )
    }

    fn a_command() -> SetBackupScheduleCommand {
        SetBackupScheduleCommand {
            organisation_id: OrganisationId(Uuid::from_u128(1)),
            deployment_id: deployment_id(),
            cadence: Cadence::Weekly {
                day: chrono::Weekday::Sun,
                at: chrono::NaiveTime::from_hms_opt(4, 0, 0).unwrap(),
            },
            zone: "Europe/Paris".parse().unwrap(),
            retention: Retention::new(std::num::NonZeroU32::new(14).unwrap(), 60).unwrap(),
            enabled: true,
        }
    }

    #[tokio::test]
    async fn a_deployment_reports_its_own_archives() {
        let mut backups = MockBackupRepository::new();
        backups.expect_list_for_deployment().returning(|_| {
            Box::pin(std::future::ready(Ok(vec![backup(
                BackupMethod::Physical,
                "26.0.0",
                17,
            )])))
        });

        let listed = reading(
            backups,
            deployments_returning_one(),
            MockBackupScheduleRepository::new(),
            Permissive,
        )
        .list_backups(
            a_caller(),
            OrganisationId(Uuid::from_u128(1)),
            deployment_id(),
        )
        .await
        .expect("the archives were listed");

        assert_eq!(listed.len(), 1);
    }

    /// The check that stops one organisation reading another's archives by
    /// pairing an organisation it belongs to with a deployment id it guessed.
    /// Reported as not found rather than refused: whether that deployment
    /// exists is not this caller's business either.
    #[tokio::test]
    async fn archives_of_another_organisation_are_not_reachable_by_guessing_an_id() {
        let refused = reading(
            MockBackupRepository::new(),
            deployments_returning_one(),
            MockBackupScheduleRepository::new(),
            Permissive,
        )
        .list_backups(
            a_caller(),
            OrganisationId(Uuid::from_u128(404)),
            deployment_id(),
        )
        .await
        .expect_err("a deployment of another organisation was listed");

        assert!(
            matches!(refused, CoreError::DeploymentNotFound { .. }),
            "got {refused}"
        );
    }

    #[tokio::test]
    async fn nobody_without_the_right_reads_a_deployments_archives() {
        let mut backups = MockBackupRepository::new();
        // The refusal comes before the read. A refusal that first fetched the
        // rows has already done the thing it is refusing.
        backups.expect_list_for_deployment().never();

        let refused = reading(
            backups,
            MockDeploymentRepository::new(),
            MockBackupScheduleRepository::new(),
            Refuses,
        )
        .list_backups(
            a_caller(),
            OrganisationId(Uuid::from_u128(1)),
            deployment_id(),
        )
        .await
        .expect_err("archives were listed without the right to see them");

        assert!(matches!(refused, CoreError::PermissionDenied { .. }));
    }

    /// A deployment created before the platform wrote schedules is archived on
    /// the same terms as one created after it. Answering "none" would describe
    /// the row rather than the deployment.
    #[tokio::test]
    async fn a_deployment_with_no_schedule_row_is_still_on_the_default() {
        let schedule = reading(
            MockBackupRepository::new(),
            deployments_returning_one(),
            schedules_holding(None),
            Permissive,
        )
        .get_backup_schedule(
            a_caller(),
            OrganisationId(Uuid::from_u128(1)),
            deployment_id(),
        )
        .await
        .expect("a schedule was answered");

        assert!(schedule.enabled);
        assert_eq!(schedule.to_cron(), "0 30 2 * * *");
    }

    #[tokio::test]
    async fn changing_the_schedule_keeps_what_the_caller_did_not_send() {
        let first_written = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let mut existing = BackupSchedule::default_for(
            deployment_id(),
            OrganisationId(Uuid::from_u128(1)),
            first_written,
        );
        existing.method = BackupMethod::Physical;

        let changed = reading(
            MockBackupRepository::new(),
            deployments_returning_one(),
            schedules_holding(Some(existing)),
            Permissive,
        )
        .set_backup_schedule(a_caller(), a_command())
        .await
        .expect("the schedule was changed");

        assert_eq!(changed.to_cron(), "0 0 4 * * 0");
        assert_eq!(changed.zone.name(), "Europe/Paris");
        assert_eq!(changed.retention.keep_for_days(), 60);
        // When a deployment was first scheduled is a fact about the
        // deployment. Rewriting it on every edit would lose it.
        assert_eq!(changed.created_at, first_written);
        assert!(changed.updated_at > first_written);
    }

    #[tokio::test]
    async fn nobody_without_the_right_changes_a_schedule() {
        let mut schedules = MockBackupScheduleRepository::new();
        schedules.expect_save().never();

        let refused = reading(
            MockBackupRepository::new(),
            MockDeploymentRepository::new(),
            schedules,
            Refuses,
        )
        .set_backup_schedule(a_caller(), a_command())
        .await
        .expect_err("a schedule was changed without the right to");

        assert!(matches!(refused, CoreError::PermissionDenied { .. }));
    }

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
            environment: crate::deployments::environment::Environment::Development,
            offer: None,
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
        }
    }

    /// Somebody with an account, as opposed to the data plane. What they may
    /// do is the policy's business, not this identity's.
    fn a_caller() -> Identity {
        Identity::User(aether_auth::User {
            id: Uuid::from_u128(7).to_string(),
            username: "somebody".to_string(),
            email: None,
            name: None,
            roles: Vec::new(),
        })
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
            // What a data plane taking an operational backup actually reports:
            // the store encrypted it, and nothing wrapped a key for it.
            protection: ArchiveProtection::StoreManaged,
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

        let service = service(
            backups,
            deployments_returning_one(),
            MockAuditRepository::new(),
        );

        let recorded = service
            .record_archive(herald(), a_report())
            .await
            .expect("recorded");

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
        backups.expect_find_by_object_key().returning(move |_, _| {
            let existing = existing.clone();
            Box::pin(async move { Ok(Some(existing)) })
        });
        // The assertion that matters: nothing is written on the second report.
        backups.expect_record().never();

        let service = service(
            backups,
            deployments_returning_one(),
            MockAuditRepository::new(),
        );

        let returned = service
            .record_archive(herald(), a_report())
            .await
            .expect("recorded");

        assert_eq!(returned.id, existing_id);
    }

    #[tokio::test]
    async fn an_archive_of_nothing_is_refused_with_a_reason() {
        let mut backups = MockBackupRepository::new();
        backups
            .expect_find_by_object_key()
            .returning(|_, _| Box::pin(async { Ok(None) }));
        backups.expect_record().never();

        let service = service(
            backups,
            deployments_returning_one(),
            MockAuditRepository::new(),
        );

        let refused = service
            .record_archive(
                herald(),
                RecordArchiveCommand {
                    size_bytes: 0,
                    ..a_report()
                },
            )
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
        let service = service(
            backups,
            deployments_returning_one(),
            MockAuditRepository::new(),
        );

        let refused = service
            .record_archive(
                herald(),
                RecordArchiveCommand {
                    finished_at: report.started_at - Duration::seconds(1),
                    ..report
                },
            )
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

        let service = service(
            backups,
            deployments_returning_one(),
            MockAuditRepository::new(),
        );

        let refused = service
            .record_archive(
                herald(),
                RecordArchiveCommand {
                    object_key: "../../someone-else/base/data.tar.gz".to_string(),
                    ..a_report()
                },
            )
            .await
            .expect_err("a report addressed another prefix");

        assert!(
            matches!(refused, CoreError::ObjectStore(_)),
            "got {refused}"
        );
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

        let service = service(
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

        let service = service(
            backups,
            deployments_returning_one(),
            MockAuditRepository::new(),
        );

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
        audit.expect_append().times(1).returning(|entry| {
            assert_eq!(entry.action.0, ARCHIVE_FAILED);
            assert_eq!(entry.target.id, Uuid::from_u128(2));
            Box::pin(async { Ok(()) })
        });

        let service = service(backups, deployments_returning_one(), audit);

        service
            .record_archive_failure(
                herald(),
                RecordArchiveFailureCommand {
                    dataplane_id: DataPlaneId(Uuid::from_u128(9)),
                    deployment_id: deployment_id(),
                    reason: "cannot proceed with the backup as the cluster has no backup section"
                        .to_string(),
                    attempted_at: Utc::now(),
                },
            )
            .await
            .expect("the attempt is recorded");
    }
}
