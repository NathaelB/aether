//! Progressive rollout, and the operator version a release requires, against
//! a real Postgres.
//!
//! The rules that matter here live in the schema -- the array CHECK
//! constraints, and the `COALESCE` a heartbeat uses to avoid erasing a
//! reported operator version -- so a mock of the driver would only confirm
//! that the code sent a string, not that the database agrees with it.
//!
//! Runs only when `DATABASE_URL` is set. See `heartbeat_activates.rs`.

use aether_domain::{
    CoreError,
    catalog::{
        BreakingRisk, Release, ReleaseId, ReleaseNotes, Rollout, RolloutPercentage,
        ports::ReleaseRepository,
    },
    dataplane::{
        entities::DataPlane,
        ports::DataPlaneRepository,
        value_objects::{Capacity, DataPlaneAllocation, Region},
    },
    deployments::DeploymentKind,
    organisation::{OrganisationId, value_objects::Plan},
    version::Version,
};
use aether_persistence::with_tx;
use aether_postgres::{catalog::PostgresReleaseRepository, dataplane::PostgresDataPlaneRepository};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

mod support;
use support::pool;

/// Each test owns a major version nobody else writes, the same convention
/// `release_catalogue.rs` follows and for the same reason: these run in
/// parallel against one shared database.
fn reserved(major: u64, patch: u64) -> Version {
    Version::parse(&format!("{major}.0.{patch}")).expect("a valid version")
}

async fn clean_releases(pool: &PgPool, major: u64) {
    sqlx::query("DELETE FROM releases WHERE version LIKE $1")
        .bind(format!("{major}.%"))
        .execute(pool)
        .await
        .expect("cleanup");
}

const DATAPLANE_REGION: &str = "release-rollout-test";

async fn clean_dataplanes(pool: &PgPool) {
    sqlx::query("DELETE FROM data_planes WHERE region = $1")
        .bind(DATAPLANE_REGION)
        .execute(pool)
        .await
        .expect("cleanup");
}

fn release(kind: DeploymentKind, version: Version) -> Release {
    Release::announce(
        ReleaseId::new(kind, version),
        BreakingRisk::None,
        ReleaseNotes("notes".to_string()),
        Utc::now(),
    )
}

/// The literal claim the adapter makes: what `Rollout` holds survives a
/// round trip through the `releases` table unchanged, order included for the
/// plans (the database has no reason to reorder a small array, but a mapping
/// bug going through a set would not preserve it either).
#[tokio::test]
async fn a_rollout_round_trips_through_the_database_unchanged() {
    let Some(pool) = pool().await else {
        return;
    };
    clean_releases(&pool, 950).await;
    let version = reserved(950, 1);
    let pilot = OrganisationId(Uuid::new_v4());

    let result: Result<Option<Release>, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let releases = PostgresReleaseRepository::new(&tx);
            let mut written = release(DeploymentKind::Ferriskey, version.clone());
            written.rollout = Rollout::new(
                RolloutPercentage::new(30).expect("valid"),
                Some(vec![Plan::Business, Plan::Enterprise]),
                vec![pilot],
            );
            releases.insert(written).await?;

            releases.get(&DeploymentKind::Ferriskey, &version).await
        },
    )
    .await;

    let found = result.expect("committed").expect("the release is there");
    clean_releases(&pool, 950).await;

    assert_eq!(
        found.rollout.percentage(),
        RolloutPercentage::new(30).unwrap()
    );
    assert_eq!(
        found.rollout.plans(),
        Some([Plan::Business, Plan::Enterprise].as_slice())
    );
    assert_eq!(found.rollout.pilot_organisations(), [pilot]);
}

/// A release with no rollout ever applied stores the same "everyone" default
/// `Rollout::full()` starts with -- there is no third state between "widened"
/// and "absent" for a column with a `NOT NULL DEFAULT`.
#[tokio::test]
async fn a_release_with_no_rollout_change_defaults_to_covering_everyone() {
    let Some(pool) = pool().await else {
        return;
    };
    clean_releases(&pool, 951).await;
    let version = reserved(951, 1);

    let result: Result<Option<Release>, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let releases = PostgresReleaseRepository::new(&tx);
            releases
                .insert(release(DeploymentKind::Ferriskey, version.clone()))
                .await?;

            releases.get(&DeploymentKind::Ferriskey, &version).await
        },
    )
    .await;

    let found = result.expect("committed").expect("the release is there");
    clean_releases(&pool, 951).await;

    assert_eq!(found.rollout, Rollout::full());
}

/// The check constraint the migration adds, exercised with a value the
/// domain would never produce -- proof the database itself refuses it, not
/// only the Rust type that normally stands in front of it.
#[tokio::test]
async fn an_unknown_plan_is_rejected_by_the_database_itself() {
    let Some(pool) = pool().await else {
        return;
    };
    clean_releases(&pool, 952).await;
    let version = reserved(952, 1);

    let outcome = sqlx::query(
        r#"
        INSERT INTO releases (kind, version, status, risk, notes, rollout_plans)
        VALUES ($1, $2, 'upcoming', 'none', '', ARRAY['gold'])
        "#,
    )
    .bind(DeploymentKind::Ferriskey.to_string())
    .bind(version.to_string())
    .execute(&pool)
    .await;

    clean_releases(&pool, 952).await;

    assert!(outcome.is_err(), "'gold' is not a plan this platform has");
}

/// Same proof for the percentage column: the CHECK is what stops a value
/// outside 0-100 from ever reaching a row, not just `RolloutPercentage::new`.
#[tokio::test]
async fn a_percentage_outside_the_range_is_rejected_by_the_database_itself() {
    let Some(pool) = pool().await else {
        return;
    };
    clean_releases(&pool, 953).await;
    let version = reserved(953, 1);

    let outcome = sqlx::query(
        r#"
        INSERT INTO releases (kind, version, status, risk, notes, rollout_percentage)
        VALUES ($1, $2, 'upcoming', 'none', '', 142)
        "#,
    )
    .bind(DeploymentKind::Ferriskey.to_string())
    .bind(version.to_string())
    .execute(&pool)
    .await;

    clean_releases(&pool, 953).await;

    assert!(outcome.is_err(), "142 is not a percentage");
}

/// The acceptance criterion of #117, one layer down: a data plane's reported
/// operator version survives a heartbeat, and a later heartbeat that does not
/// carry one leaves it exactly where it was rather than erasing it.
#[tokio::test]
async fn a_heartbeat_records_the_operator_version_and_a_silent_one_does_not_erase_it() {
    let Some(pool) = pool().await else {
        return;
    };
    clean_dataplanes(&pool).await;

    let result: Result<Option<DataPlane>, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let planes = PostgresDataPlaneRepository::new(&tx);
            let dataplane = DataPlane::new(
                DataPlaneAllocation::Shared,
                Region::new(DATAPLANE_REGION),
                Capacity::new(4_000, 8_192, 100).expect("non-zero capacity"),
            );
            planes.save(&dataplane).await?;

            planes
                .touch_last_seen(&dataplane.id, Utc::now(), Some(Version::new(1, 4, 0)))
                .await?;
            // A later heartbeat that reports nothing must not erase what the
            // first one recorded.
            planes
                .touch_last_seen(&dataplane.id, Utc::now(), None)
                .await?;

            planes.find_by_id(&dataplane.id).await
        },
    )
    .await;

    let found = result.expect("committed").expect("the data plane is there");
    clean_dataplanes(&pool).await;

    assert_eq!(found.operator_version, Some(Version::new(1, 4, 0)));
}
