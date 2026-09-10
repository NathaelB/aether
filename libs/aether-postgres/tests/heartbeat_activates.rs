//! What a heartbeat does to a data plane's status, checked against a real
//! Postgres rather than a mock.
//!
//! The rule this covers lives in SQL -- a `CASE` inside the `UPDATE` -- so no
//! domain unit test can reach it, and a mock of the driver would only assert
//! that a string was sent. This is the shape of logic that has to meet the real
//! engine.
//!
//! **Runs only when `DATABASE_URL` is set**, and skips loudly otherwise. There
//! is no Postgres in CI yet, so making it mandatory would turn every run red.
//! With one:
//!
//! ```sh
//! DATABASE_URL=postgres://aether:aether@localhost:5433/aether \
//!   cargo test -p aether-postgres --test heartbeat_activates
//! ```

use aether_domain::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        ports::DataPlaneRepository,
        value_objects::{Capacity, DataPlaneAllocation, DataPlaneStatus, Region},
    },
};
use aether_persistence::with_tx;
use aether_postgres::dataplane::PostgresDataPlaneRepository;
use chrono::Utc;
use sqlx::PgPool;

mod support;
use support::pool;

/// Rows written by this test carry a region nothing else uses, so cleanup can
/// be unambiguous and the test can run against a database with real rows in it.
const TEST_REGION: &str = "heartbeat-activates-test";

/// Saves a data plane in `initial`, heartbeats it, and returns the status it
/// ended up in.
///
/// `with_tx` commits, so the rows are deleted afterwards rather than rolled
/// back -- it is the crate's public entry point and the one production uses,
/// which is worth more here than the convenience of an automatic rollback.
async fn status_after_heartbeat(pool: &PgPool, initial: DataPlaneStatus) -> DataPlaneStatus {
    let result = with_tx(
        pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let repository = PostgresDataPlaneRepository::new(&tx);

            let mut dataplane = DataPlane::new(
                DataPlaneAllocation::Shared,
                Region::new(TEST_REGION),
                Capacity::new(4_000, 8_192, 100).expect("non-zero capacity"),
            );
            dataplane.status = initial;
            repository.save(&dataplane).await?;

            let touched = repository
                .touch_last_seen(&dataplane.id, Utc::now())
                .await?;
            assert!(touched, "the data plane was just saved, so it exists");

            let reloaded = repository
                .find_by_id(&dataplane.id)
                .await?
                .expect("the data plane was just saved");

            assert!(
                reloaded.last_seen_at.is_some(),
                "every heartbeat stamps last_seen_at, whatever the status"
            );

            Ok(reloaded.status)
        },
    )
    .await;

    result.expect("the transaction committed")
}

async fn cleanup(pool: &PgPool) {
    sqlx::query("DELETE FROM data_planes WHERE region = $1")
        .bind(TEST_REGION)
        .execute(pool)
        .await
        .expect("cleanup");
}

/// The bug this exists for.
///
/// Registering a data plane leaves it `Provisioning`, `find_available` requires
/// `active`, and nothing else in the system made the transition -- so a shared
/// data plane could be registered, heartbeat happily, and never be placed on.
/// Shared placement was unreachable, and no test noticed because the rule that
/// broke it lives in SQL.
///
/// A heartbeat is the only evidence the control plane ever gets that a cluster
/// finished coming up: it means Herald is running inside it and talking.
#[tokio::test]
async fn a_heartbeat_promotes_a_provisioning_data_plane_to_active() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let status = status_after_heartbeat(&pool, DataPlaneStatus::Provisioning).await;
    cleanup(&pool).await;

    assert_eq!(status, DataPlaneStatus::Active);
}

/// Draining and disabled are decisions an operator made, and a cluster being
/// drained keeps reporting the whole time it drains -- promoting it would undo
/// the drain on the very next heartbeat. Failed records that provisioning did
/// not complete, and reviving it silently would hide a half-built cluster.
#[tokio::test]
async fn a_heartbeat_never_overrides_a_status_someone_chose() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    for chosen in [
        DataPlaneStatus::Draining,
        DataPlaneStatus::Disabled,
        DataPlaneStatus::Failed,
    ] {
        let status = status_after_heartbeat(&pool, chosen).await;

        assert_eq!(status, chosen, "{chosen:?} must survive a heartbeat");
    }

    cleanup(&pool).await;
}

/// Already active stays active: the promotion is a transition, not a rewrite.
#[tokio::test]
async fn a_heartbeat_on_an_active_data_plane_changes_nothing_about_its_status() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let status = status_after_heartbeat(&pool, DataPlaneStatus::Active).await;
    cleanup(&pool).await;

    assert_eq!(status, DataPlaneStatus::Active);
}
