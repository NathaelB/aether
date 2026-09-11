//! What the catalogue table guarantees on its own.
//!
//! The rules here are the primary key and the check constraints, so they are
//! checked against a real Postgres. A mock would only confirm that the code
//! calls what I wrote, which is not the question.
//!
//! Runs only when `DATABASE_URL` is set. See `heartbeat_activates.rs`.

use aether_domain::{
    CoreError,
    catalog::{
        BreakingRisk, Release, ReleaseId, ReleaseNotes, ReleaseStatus, ports::ReleaseRepository,
    },
    deployments::DeploymentKind,
    version::Version,
};
use aether_persistence::with_tx;
use aether_postgres::catalog::PostgresReleaseRepository;
use chrono::Utc;
use sqlx::PgPool;

mod support;
use support::pool;

fn release(kind: DeploymentKind, version: Version) -> Release {
    Release::announce(
        ReleaseId::new(kind, version),
        BreakingRisk::None,
        ReleaseNotes("notes".to_string()),
        Utc::now(),
    )
}

/// Each test owns a major version nobody else writes.
///
/// Reserving patch numbers was not enough: these run in parallel against a
/// shared database, and a cleanup keyed on the shared prefix deleted another
/// test's rows while it was still reading them.
fn reserved(major: u64, patch: u64) -> Version {
    Version::parse(&format!("{major}.0.{patch}")).expect("a valid version")
}

async fn clean(pool: &PgPool, major: u64) {
    sqlx::query("DELETE FROM releases WHERE version LIKE $1")
        .bind(format!("{major}.%"))
        .execute(pool)
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn a_release_survives_a_round_trip() {
    let Some(pool) = pool().await else {
        return;
    };
    clean(&pool, 991).await;
    let version = reserved(991, 1);

    let result: Result<Option<Release>, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let releases = PostgresReleaseRepository::new(&tx);
            let mut written = release(DeploymentKind::Ferriskey, version.clone());
            written.revise(
                BreakingRisk::Breaking,
                ReleaseNotes("changes a default".to_string()),
                Vec::new(),
                None,
                Utc::now(),
            );
            releases.insert(written).await?;

            releases.get(&DeploymentKind::Ferriskey, &version).await
        },
    )
    .await;

    let found = result.expect("committed").expect("the release is there");
    clean(&pool, 991).await;

    assert_eq!(found.risk, BreakingRisk::Breaking);
    assert_eq!(found.notes.0, "changes a default");
    assert_eq!(found.status, ReleaseStatus::Upcoming);
}

/// Two rows for one release is two answers to "may this be installed". The
/// primary key is what makes that unrepresentable, rather than a check the
/// caller has to remember.
#[tokio::test]
async fn the_same_release_cannot_be_published_twice() {
    let Some(pool) = pool().await else {
        return;
    };
    clean(&pool, 992).await;
    let version = reserved(992, 2);

    let result: Result<CoreError, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let releases = PostgresReleaseRepository::new(&tx);
            releases
                .insert(release(DeploymentKind::Ferriskey, version.clone()))
                .await?;

            let second = releases
                .insert(release(DeploymentKind::Ferriskey, version.clone()))
                .await
                .expect_err("the catalogue already holds it");

            Ok(second)
        },
    )
    .await;

    let error = result.expect("committed");
    clean(&pool, 992).await;

    assert!(
        matches!(error, CoreError::ReleaseAlreadyExists { .. }),
        "got {error:?}"
    );
    assert!(error.to_string().contains("992.0.2"), "{error}");
}

/// The same version number for two products is two different releases.
#[tokio::test]
async fn two_products_can_hold_the_same_version() {
    let Some(pool) = pool().await else {
        return;
    };
    clean(&pool, 993).await;
    let version = reserved(993, 3);

    let result: Result<(), CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let releases = PostgresReleaseRepository::new(&tx);
            releases
                .insert(release(DeploymentKind::Ferriskey, version.clone()))
                .await?;
            releases
                .insert(release(DeploymentKind::Keycloak, version.clone()))
                .await
        },
    )
    .await;

    clean(&pool, 993).await;
    result.expect("both products hold their own release");
}

/// Notes are prose written by a human. The one thing worse than long notes is
/// notes cut off mid-sentence, which is what a VARCHAR would have done.
#[tokio::test]
async fn long_notes_are_stored_whole() {
    let Some(pool) = pool().await else {
        return;
    };
    clean(&pool, 994).await;
    let version = reserved(994, 4);
    let long = "a".repeat(40_000);

    let result: Result<Option<Release>, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let releases = PostgresReleaseRepository::new(&tx);
            let mut written = release(DeploymentKind::Ferriskey, version.clone());
            written.notes = ReleaseNotes(long.clone());
            releases.insert(written).await?;

            releases.get(&DeploymentKind::Ferriskey, &version).await
        },
    )
    .await;

    let found = result.expect("committed").expect("the release is there");
    clean(&pool, 994).await;

    assert_eq!(found.notes.0.len(), 40_000, "the notes were truncated");
}

/// Listing sorts on the parsed version, not on the text column. Postgres would
/// put 999.0.10 before 999.0.9, which is the ordering bug the Version type
/// exists to prevent, reintroduced by an ORDER BY.
#[tokio::test]
async fn releases_come_back_newest_first() {
    let Some(pool) = pool().await else {
        return;
    };
    clean(&pool, 995).await;

    let result: Result<Vec<String>, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let releases = PostgresReleaseRepository::new(&tx);
            for patch in [9_u64, 10, 11] {
                releases
                    .insert(release(DeploymentKind::Keycloak, reserved(995, patch)))
                    .await?;
            }

            let listed = releases.list_for_kind(&DeploymentKind::Keycloak).await?;
            Ok(listed
                .iter()
                .map(|release| release.id.version.to_string())
                .filter(|version| version.starts_with("995."))
                .collect())
        },
    )
    .await;

    let listed = result.expect("committed");
    clean(&pool, 995).await;

    assert_eq!(listed, ["995.0.11", "995.0.10", "995.0.9"]);
}

/// A status change is written back, and the identity is not touched.
#[tokio::test]
async fn a_status_change_is_persisted() {
    let Some(pool) = pool().await else {
        return;
    };
    clean(&pool, 996).await;
    let version = reserved(996, 5);

    let result: Result<Option<Release>, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let releases = PostgresReleaseRepository::new(&tx);
            let mut written = release(DeploymentKind::Ferriskey, version.clone());
            releases.insert(written.clone()).await?;

            written
                .move_to(ReleaseStatus::Available, Utc::now())
                .expect("a forward step");
            releases.update(&written).await?;

            releases.get(&DeploymentKind::Ferriskey, &version).await
        },
    )
    .await;

    let found = result.expect("committed").expect("the release is there");
    clean(&pool, 996).await;

    assert_eq!(found.status, ReleaseStatus::Available);
    assert_eq!(found.id.version, version);
}

/// Updating something the catalogue does not hold is a mistake worth hearing
/// about, not a write that quietly affects nothing.
#[tokio::test]
async fn updating_a_release_that_is_not_there_is_refused() {
    let Some(pool) = pool().await else {
        return;
    };
    clean(&pool, 997).await;

    let result: Result<CoreError, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let releases = PostgresReleaseRepository::new(&tx);
            let absent = release(DeploymentKind::Ferriskey, reserved(997, 6));

            Ok(releases
                .update(&absent)
                .await
                .expect_err("nothing to update"))
        },
    )
    .await;

    let error = result.expect("committed");
    assert!(
        matches!(error, CoreError::ReleaseNotFound { .. }),
        "got {error:?}"
    );
}

/// The steps a path must pass through are declared prose from whoever
/// publishes the release, in the order they wrote them. Order is the part a
/// naive mapping loses first, so it is asserted explicitly rather than with a
/// set comparison.
#[tokio::test]
async fn a_release_round_trips_its_steps_through_unchanged() {
    let Some(pool) = pool().await else {
        return;
    };
    clean(&pool, 998).await;
    let version = reserved(998, 7);
    let steps = vec![
        Version::parse("998.0.3").expect("valid"),
        Version::parse("998.0.5").expect("valid"),
        Version::parse("998.0.6").expect("valid"),
    ];
    let minimum_operator = Version::parse("1.4.0").expect("valid");

    let result: Result<Option<Release>, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let releases = PostgresReleaseRepository::new(&tx);
            let mut written = release(DeploymentKind::Ferriskey, version.clone());
            written.steps_through = steps.clone();
            written.minimum_operator_version = Some(minimum_operator.clone());
            releases.insert(written).await?;

            releases.get(&DeploymentKind::Ferriskey, &version).await
        },
    )
    .await;

    let found = result.expect("committed").expect("the release is there");
    clean(&pool, 998).await;

    assert_eq!(found.steps_through, steps, "the order must survive");
    assert_eq!(found.minimum_operator_version, Some(minimum_operator));
}
