//! Archives and their schedule, against the real tables.
//!
//! The unit tests cover the rules. This covers what they cannot: that the
//! domain's shape and the database's shape are the same shape in both
//! directions, and that the invariants the domain states in types are also
//! enforced against anything writing around it.
//!
//! Runs only when `DATABASE_URL` is set. See `heartbeat_activates.rs`.

use std::num::{NonZeroU32, NonZeroU64};

use aether_domain::{
    CoreError,
    backups::{
        ArchivePrefix, Backup, BackupId, BackupMethod, BackupSchedule, Cadence, PostgresMajor,
        Retention,
        keys::{KeyName, KeyRef, KeyVersion, ProviderName},
        ports::{BackupRepository, BackupScheduleRepository},
    },
    catalog::ReleaseId,
    dataplane::{
        entities::DataPlane,
        ports::DataPlaneRepository,
        value_objects::{Capacity, DataPlaneAllocation, DataPlaneId, DeploymentResources, Region},
    },
    deployments::{
        Deployment, DeploymentId, DeploymentKind, DeploymentName, DeploymentStatus,
        network::NetworkAccess, ports::DeploymentRepository,
    },
    organisation::OrganisationId,
    upgrades::policy::AutoUpgradePolicy,
    user::UserId,
    version::Version,
};
use aether_persistence::with_tx;
use aether_postgres::{
    backups::{PostgresBackupRepository, PostgresBackupScheduleRepository},
    dataplane::PostgresDataPlaneRepository,
    deployments::PostgresDeploymentRepository,
};
use chrono::{NaiveTime, TimeZone, Utc, Weekday};
use uuid::Uuid;

mod support;
use support::pool;

const TEST_REGION: &str = "backups-test";

macro_rules! pool_or_skip {
    () => {
        match pool().await {
            Some(pool) => pool,
            None => {
                eprintln!("skipped: DATABASE_URL is not set");
                return;
            }
        }
    };
}

#[tokio::test]
async fn a_schedule_survives_the_round_trip() {
    let pool = pool_or_skip!();

    let read: Result<Option<BackupSchedule>, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let deployment = seed(&tx).await?;
            let schedules = PostgresBackupScheduleRepository::new(&tx);

            let schedule = BackupSchedule {
                deployment_id: deployment.id,
                organisation_id: deployment.organisation_id,
                cadence: Cadence::Weekly {
                    day: Weekday::Sun,
                    at: NaiveTime::from_hms_opt(3, 15, 0).expect("a time"),
                },
                zone: chrono_tz::Europe::Paris,
                retention: Retention::new(NonZeroU32::new(5).unwrap(), 45)?,
                method: BackupMethod::Logical,
                enabled: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            schedules.save(schedule).await?;

            schedules.get(&deployment.id).await
        },
    )
    .await;

    let back = read.expect("the transaction").expect("the schedule");

    assert_eq!(
        back.cadence,
        Cadence::Weekly {
            day: Weekday::Sun,
            at: NaiveTime::from_hms_opt(3, 15, 0).unwrap(),
        }
    );
    // A real zone rather than the offset it happened to have when it was
    // written. 03:15 in Paris stays 03:15 after a daylight saving change.
    assert_eq!(back.zone, chrono_tz::Europe::Paris);
    assert_eq!(back.retention.keep_last().get(), 5);
    assert_eq!(back.retention.keep_for_days(), 45);
    assert_eq!(back.method, BackupMethod::Logical);
    assert_eq!(back.to_cron(), "CRON_TZ=Europe/Paris 0 15 3 * * 0");
}

#[tokio::test]
async fn saving_a_schedule_again_replaces_it() {
    let pool = pool_or_skip!();

    let read: Result<Option<BackupSchedule>, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let deployment = seed(&tx).await?;
            let schedules = PostgresBackupScheduleRepository::new(&tx);

            let mut schedule =
                BackupSchedule::default_for(deployment.id, deployment.organisation_id, Utc::now());
            schedules.save(schedule.clone()).await?;

            // A deployment with two schedules would be two answers to "when is
            // this backed up". The second save has to land on the same row.
            schedule.enabled = false;
            schedule.retention = Retention::new(NonZeroU32::new(2).unwrap(), 3)?;
            schedules.save(schedule).await?;

            schedules.get(&deployment.id).await
        },
    )
    .await;

    let back = read.expect("the transaction").expect("the schedule");

    assert!(!back.enabled);
    assert_eq!(back.retention.keep_last().get(), 2);
}

#[tokio::test]
async fn a_schedule_that_is_off_is_not_listed() {
    let pool = pool_or_skip!();

    let listed: Result<(bool, bool), CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let on = seed(&tx).await?;
            let off = seed(&tx).await?;
            let schedules = PostgresBackupScheduleRepository::new(&tx);

            schedules
                .save(BackupSchedule::default_for(
                    on.id,
                    on.organisation_id,
                    Utc::now(),
                ))
                .await?;

            let mut disabled = BackupSchedule::default_for(off.id, off.organisation_id, Utc::now());
            disabled.enabled = false;
            schedules.save(disabled).await?;

            let enabled = schedules.list_enabled().await?;

            Ok((
                enabled.iter().any(|s| s.deployment_id == on.id),
                enabled.iter().any(|s| s.deployment_id == off.id),
            ))
        },
    )
    .await;

    let (on_is_listed, off_is_listed) = listed.expect("the transaction");

    assert!(on_is_listed);
    assert!(
        !off_is_listed,
        "a schedule that is off was handed to the scheduler"
    );
}

#[tokio::test]
async fn an_archive_survives_the_round_trip() {
    let pool = pool_or_skip!();

    let read: Result<Option<Backup>, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let deployment = seed(&tx).await?;
            let backups = PostgresBackupRepository::new(&tx);

            let backup = archive(&deployment, "base/20260912T0230Z.tar", 0);
            let id = backup.id;
            backups.record(backup).await?;

            backups.get(&id).await
        },
    )
    .await;

    let back = read.expect("the transaction").expect("the archive");

    assert_eq!(back.method, BackupMethod::Physical);
    assert_eq!(back.postgres_major, PostgresMajor(17));
    assert_eq!(back.size_bytes.get(), 4096);
    assert_eq!(back.key.version, KeyVersion::new(3));
    assert_eq!(back.key.name.as_str(), "aether-backups");
    // Rebuilt through the prefix rather than read back as a stored path, so a
    // row whose columns disagreed with its own key could not produce a
    // location at all.
    assert_eq!(
        back.location.as_path(),
        format!(
            "{}/{}/base/20260912T0230Z.tar",
            back.organisation_id, back.deployment_id
        )
    );
}

#[tokio::test]
async fn archives_come_back_newest_first() {
    let pool = pool_or_skip!();

    let listed: Result<Vec<i64>, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let deployment = seed(&tx).await?;
            let backups = PostgresBackupRepository::new(&tx);

            // Written oldest first on purpose: insertion order must not be
            // what decides the answer.
            for days_ago in [5, 1, 3] {
                backups
                    .record(archive(
                        &deployment,
                        &format!("base/{days_ago}.tar"),
                        days_ago,
                    ))
                    .await?;
            }

            Ok(backups
                .list_for_deployment(&deployment.id)
                .await?
                .iter()
                .map(|backup| (Utc::now() - backup.finished_at).num_days())
                .collect())
        },
    )
    .await;

    assert_eq!(listed.expect("the transaction"), vec![1, 3, 5]);
}

#[tokio::test]
async fn a_forgotten_archive_is_gone() {
    let pool = pool_or_skip!();

    let read: Result<Option<Backup>, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let deployment = seed(&tx).await?;
            let backups = PostgresBackupRepository::new(&tx);

            let backup = archive(&deployment, "base/one.tar", 0);
            let id = backup.id;
            backups.record(backup).await?;
            backups.forget(&id).await?;

            backups.get(&id).await
        },
    )
    .await;

    assert!(read.expect("the transaction").is_none());
}

/// The domain says this with `NonZeroU64`. The column says it too, because the
/// domain is where a person gets a usable message and the constraint is what
/// holds when something writes around it.
#[tokio::test]
async fn the_database_refuses_an_archive_of_nothing() {
    let pool = pool_or_skip!();

    let refused: Result<bool, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let deployment = seed(&tx).await?;
            let mut guard = tx.lock().await;

            let result = sqlx::query(
                "INSERT INTO backups (id, deployment_id, organisation_id, kind, version, \
                 postgres_major, method, key_provider, key_name, key_version, object_key, \
                 size_bytes, started_at, finished_at) \
                 VALUES ($1, $2, $3, 'keycloak', '26.0.0', 17, 'physical', 'platform', \
                 'aether-backups', 1, 'base/empty.tar', 0, now(), now())",
            )
            .bind(Uuid::new_v4())
            .bind(deployment.id.0)
            .bind(deployment.organisation_id.0)
            .execute(&mut ***guard)
            .await;

            Ok(result.is_err())
        },
    )
    .await;

    // The transaction itself fails once the constraint fires, which is the
    // outcome being asserted either way.
    match refused {
        Ok(was_refused) => assert!(was_refused, "an archive of zero bytes was stored"),
        Err(CoreError::DatabaseError { message }) => {
            assert!(
                message.contains("backups_are_not_empty"),
                "refused for the wrong reason: {message}"
            );
        }
        Err(other) => panic!("refused for the wrong reason: {other}"),
    }
}

fn archive(deployment: &Deployment, key: &str, days_ago: i64) -> Backup {
    let finished_at = Utc::now() - chrono::Duration::days(days_ago);

    Backup {
        id: BackupId(Uuid::new_v4()),
        deployment_id: deployment.id,
        organisation_id: deployment.organisation_id,
        release: ReleaseId::new(DeploymentKind::Keycloak, Version::parse("26.0.0").unwrap()),
        postgres_major: PostgresMajor(17),
        method: BackupMethod::Physical,
        key: KeyRef::new(
            ProviderName::platform(),
            KeyName::new("aether-backups").unwrap(),
            KeyVersion::new(3),
        ),
        location: ArchivePrefix::new(deployment.organisation_id, deployment.id)
            .object(key)
            .expect("a key inside the prefix"),
        size_bytes: NonZeroU64::new(4096).unwrap(),
        started_at: finished_at - chrono::Duration::minutes(30),
        finished_at,
    }
}

async fn seed(tx: &aether_persistence::SharedTx<'_>) -> Result<Deployment, CoreError> {
    let dataplanes = PostgresDataPlaneRepository::new(tx);
    let deployments = PostgresDeploymentRepository::new(tx);

    let dataplane = DataPlane::new(
        DataPlaneAllocation::Shared,
        Region::new(TEST_REGION),
        Capacity::new(8_000, 16_384, 200).expect("non-zero capacity"),
    );
    dataplanes.save(&dataplane).await?;

    let organisation_id = OrganisationId(Uuid::new_v4());
    let user_id = UserId(Uuid::new_v4());
    {
        let mut guard = tx.lock().await;
        sqlx::query("INSERT INTO users (id, email, name, sub) VALUES ($1, $2, 'backups', $3)")
            .bind(user_id.0)
            .bind(format!("{}@backups.test", user_id.0))
            .bind(user_id.0.to_string())
            .execute(&mut ***guard)
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;

        sqlx::query(
            "INSERT INTO organisations \
             (id, name, slug, owner_id, status, plan, max_instances, max_users, \
              max_storage_gb, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, 'active', 'free', 5, 5, 5, now(), now())",
        )
        .bind(organisation_id.0)
        .bind(organisation_id.0.to_string())
        .bind(organisation_id.0.to_string())
        .bind(user_id.0)
        .execute(&mut ***guard)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: e.to_string(),
        })?;
    }

    let now = Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap();
    let deployment = Deployment {
        id: DeploymentId(Uuid::new_v4()),
        organisation_id,
        dataplane_id: DataPlaneId(dataplane.id.0),
        name: DeploymentName("backups".to_string()),
        kind: DeploymentKind::Keycloak,
        version: Version::parse("26.0.0").unwrap(),
        status: DeploymentStatus::Successful,
        namespace: "backups-test".to_string(),
        resources: DeploymentResources::DEFAULT,
        created_by: user_id,
        created_at: now,
        updated_at: now,
        deployed_at: None,
        deleted_at: None,
        auto_upgrade: AutoUpgradePolicy::Manual,
        maintenance_window: None,
        network_access: NetworkAccess::Open,
    };
    deployments.insert(deployment.clone()).await?;

    Ok(deployment)
}
