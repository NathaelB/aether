//! No deployment can hold a version the domain cannot read.
//!
//! Two things enforce this and either one alone leaves a hole. The API refuses
//! an unparsable version on the way in, but nothing routes a migration, a
//! fixture or a hand-written UPDATE through the API. The database constraint
//! catches those, and it is SQL, so it is checked against a real Postgres
//! rather than a mock.
//!
//! Runs only when `DATABASE_URL` is set. See `heartbeat_activates.rs`.

use aether_domain::version::Version;
use sqlx::{Acquire, PgPool, Row};
use uuid::Uuid;

mod support;
use support::pool;

/// The rows that were there before versions were real. If any survived the
/// migration unparsed, everything that reads a deployment breaks at runtime
/// instead of here.
#[tokio::test]
async fn no_deployment_holds_a_version_the_domain_cannot_read() {
    let Some(pool) = pool().await else {
        return;
    };

    let rows = sqlx::query("SELECT id, version FROM deployments")
        .fetch_all(&pool)
        .await
        .expect("deployments are readable");

    let unreadable: Vec<String> = rows
        .iter()
        .filter_map(|row| {
            let raw: String = row.get("version");
            Version::parse(&raw).err().map(|_| {
                let id: Uuid = row.get("id");
                format!("{id} holds '{raw}'")
            })
        })
        .collect();

    assert!(
        unreadable.is_empty(),
        "{} deployment(s) hold an unreadable version: {}",
        unreadable.len(),
        unreadable.join(", ")
    );
}

/// The constraint, exercised on a row this test owns.
///
/// Everything happens inside one transaction that is rolled back, and each
/// candidate gets a savepoint of its own: a failed statement aborts a Postgres
/// transaction, so without one the second candidate would fail for the wrong
/// reason.
#[tokio::test]
async fn the_database_refuses_a_version_that_is_not_a_semver() {
    let Some(pool) = pool().await else {
        return;
    };

    let mut tx = pool.begin().await.expect("a transaction");
    let deployment_id = insert_fixture(&mut tx).await;

    for candidate in ["latest", "", "26", "26.0", "v26.0.1", "nightly", "26.0.1a"] {
        let mut savepoint = tx.begin().await.expect("a savepoint");

        let outcome = sqlx::query("UPDATE deployments SET version = $1 WHERE id = $2")
            .bind(candidate)
            .bind(deployment_id)
            .execute(&mut *savepoint)
            .await;

        savepoint.rollback().await.expect("rolled back");

        assert!(
            outcome.is_err(),
            "the database accepted version '{candidate}'"
        );
    }

    // And the constraint is not simply rejecting everything.
    let mut savepoint = tx.begin().await.expect("a savepoint");
    let accepted = sqlx::query("UPDATE deployments SET version = '26.0.1' WHERE id = $1")
        .bind(deployment_id)
        .execute(&mut *savepoint)
        .await;
    savepoint.rollback().await.expect("rolled back");

    assert!(accepted.is_ok(), "a real version was refused");

    tx.rollback().await.expect("rolled back");
}

/// A deployment needs an organisation, a user and a data plane. Inserted
/// directly: this test is about one column, and going through the repositories
/// would drag in the version validation it exists to check independently.
async fn insert_fixture(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Uuid {
    let tag = format!("version-check-{}", Uuid::new_v4());
    let user_id = Uuid::new_v4();
    let organisation_id = Uuid::new_v4();
    let dataplane_id = Uuid::new_v4();
    let deployment_id = Uuid::new_v4();

    sqlx::query("INSERT INTO users (id, email, name, sub) VALUES ($1, $2, $3, $4)")
        .bind(user_id)
        .bind(format!("{user_id}@version.test"))
        .bind(&tag)
        .bind(user_id.to_string())
        .execute(&mut **tx)
        .await
        .expect("a user");

    sqlx::query(
        "INSERT INTO organisations \
         (id, name, slug, owner_id, status, plan, max_instances, max_users, max_storage_gb, \
          created_at, updated_at) \
         VALUES ($1, $2, $3, $4, 'active', 'free', 1, 1, 1, now(), now())",
    )
    .bind(organisation_id)
    .bind(&tag)
    .bind(organisation_id.to_string())
    .bind(user_id)
    .execute(&mut **tx)
    .await
    .expect("an organisation");

    sqlx::query(
        "INSERT INTO data_planes \
         (id, mode, region, status, capacity_cpu_millis, capacity_memory_mib, capacity_storage_gib) \
         VALUES ($1, 'shared', $2, 'active', 8000, 16384, 200)",
    )
    .bind(dataplane_id)
    .bind(&tag)
    .execute(&mut **tx)
    .await
    .expect("a data plane");

    sqlx::query(
        "INSERT INTO deployments \
         (id, organisation_id, dataplane_id, name, kind, status, namespace, version, created_by, \
          created_at, updated_at, cpu_millis, memory_mib, storage_gib) \
         VALUES ($1, $2, $3, 'version-check', 'ferriskey', 'pending', 'ns', '26.0.1', $4, \
                 now(), now(), 500, 1024, 1)",
    )
    .bind(deployment_id)
    .bind(organisation_id)
    .bind(dataplane_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await
    .expect("a deployment");

    deployment_id
}

/// The value the platform used to write, kept as its own case: it is the one
/// somebody will type again by hand.
#[tokio::test]
async fn latest_is_refused_on_both_sides() {
    let Some(pool) = pool().await else {
        return;
    };

    assert!(
        Version::parse("latest").is_err(),
        "the domain refuses latest"
    );

    assert!(
        refused_by_the_database(&pool, "latest").await,
        "the database refuses latest"
    );
}

async fn refused_by_the_database(pool: &PgPool, candidate: &str) -> bool {
    let mut tx = pool.begin().await.expect("a transaction");
    let deployment_id = insert_fixture(&mut tx).await;

    let outcome = sqlx::query("UPDATE deployments SET version = $1 WHERE id = $2")
        .bind(candidate)
        .bind(deployment_id)
        .execute(&mut *tx)
        .await;

    tx.rollback().await.expect("rolled back");

    outcome.is_err()
}
