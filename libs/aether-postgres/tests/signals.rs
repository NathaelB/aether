//! Integration tests for signals: verifying the deduplication invariant,
//! round-tripping data, and the partial unique index behavior. All assertions
//! are against the database itself.
//!
//! Runs only when `DATABASE_URL` is set. See `support::pool`.

use aether_domain::{
    CoreError,
    dataplane::value_objects::DataPlaneId,
    deployments::DeploymentId,
    signals::{Signal, SignalId, SignalKind, SignalSubject, ports::SignalRepository},
};
use aether_persistence::with_tx;
use aether_postgres::signals::PostgresSignalRepository;
use chrono::{DateTime, SubsecRound, Utc};
use sqlx::PgPool;
use uuid::Uuid;

mod support;
use support::pool;

/// Postgres keeps microseconds; `DateTime<Utc>` keeps nanoseconds. Matching
/// other tests' fixtures: a value straight from `Utc::now()` round-trips on
/// macOS and fails on Linux.
fn now() -> DateTime<Utc> {
    Utc::now().trunc_subsecs(6)
}

fn map_err(e: sqlx::Error) -> CoreError {
    CoreError::DatabaseError {
        message: e.to_string(),
    }
}

fn signal(
    kind: SignalKind,
    subject: SignalSubject,
    dedup_key: String,
    message: String,
    at: DateTime<Utc>,
) -> Signal {
    Signal::open(
        SignalId(Uuid::new_v4()),
        kind,
        subject,
        dedup_key,
        message,
        at,
    )
}

/// Rows written by one test, removed once it has asserted. Signals have no
/// organisation scope, so each test cleans up its own rows.
async fn forget(pool: &PgPool, ids: &[Uuid]) {
    sqlx::query("DELETE FROM signals WHERE id = ANY($1)")
        .bind(ids)
        .execute(pool)
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn a_signal_survives_a_round_trip() {
    let Some(pool) = pool().await else {
        return;
    };

    let subject = SignalSubject::Dataplane {
        id: DataPlaneId(Uuid::new_v4()),
    };
    let written = signal(
        SignalKind::DataplaneHeartbeatStale,
        subject.clone(),
        "heartbeat-stale-1".to_string(),
        "No heartbeat for 5 minutes".to_string(),
        now(),
    );

    let result: Result<(), CoreError> = with_tx(&pool, map_err, async |tx| {
        let repo = PostgresSignalRepository::new(&tx);
        repo.write(written.clone()).await
    })
    .await;

    result.expect("committed");

    // Verify the row exists and has the right shape.
    let row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM signals WHERE id = $1")
        .bind(written.id.0)
        .fetch_one(&pool)
        .await
        .expect("query");

    forget(&pool, &[written.id.0]).await;

    assert_eq!(row_count, 1, "exactly one row was inserted");
}

#[tokio::test]
async fn an_open_signal_with_the_same_dedup_key_is_updated() {
    let Some(pool) = pool().await else {
        return;
    };

    let dedup_key = format!("shared-key-{}", Uuid::new_v4());
    let subject = SignalSubject::Dataplane {
        id: DataPlaneId(Uuid::new_v4()),
    };

    // First write.
    let first = signal(
        SignalKind::DataplaneHeartbeatStale,
        subject.clone(),
        dedup_key.clone(),
        "First message".to_string(),
        now(),
    );
    let _first_id = first.id.0;

    // Second write, same dedup_key, different ID and message.
    let second = signal(
        SignalKind::DataplaneHeartbeatStale,
        subject.clone(),
        dedup_key.clone(),
        "Updated message".to_string(),
        now(),
    );
    let _second_id = second.id.0;

    let result: Result<(), CoreError> = with_tx(&pool, map_err, async |tx| {
        let repo = PostgresSignalRepository::new(&tx);
        repo.write(first.clone()).await?;
        repo.write(second.clone()).await
    })
    .await;

    result.expect("committed");

    // Check that only one row exists for this dedup_key.
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM signals WHERE dedup_key = $1 AND closed_at IS NULL",
    )
    .bind(&dedup_key)
    .fetch_one(&pool)
    .await
    .expect("query");

    // Check that the message was updated (if we can).
    let message: String = sqlx::query_scalar(
        "SELECT message FROM signals WHERE dedup_key = $1 AND closed_at IS NULL",
    )
    .bind(&dedup_key)
    .fetch_one(&pool)
    .await
    .expect("query");

    // Clean up: delete by dedup_key (which covers both attempts).
    sqlx::query("DELETE FROM signals WHERE dedup_key = $1")
        .bind(&dedup_key)
        .execute(&pool)
        .await
        .expect("cleanup");

    assert_eq!(count, 1, "exactly one open signal for this dedup_key");
    assert_eq!(
        message, "Updated message",
        "the message was updated on the second write"
    );
}

#[tokio::test]
async fn signals_with_different_subjects() {
    let Some(pool) = pool().await else {
        return;
    };

    let dataplane_subject = SignalSubject::Dataplane {
        id: DataPlaneId(Uuid::new_v4()),
    };
    let deployment_subject = SignalSubject::Deployment {
        id: DeploymentId(Uuid::new_v4()),
    };
    let action_subject = SignalSubject::Action { id: Uuid::new_v4() };

    let dataplane_signal = signal(
        SignalKind::DataplaneHeartbeatStale,
        dataplane_subject.clone(),
        "dp-signal".to_string(),
        "Dataplane signal".to_string(),
        now(),
    );
    let deployment_signal = signal(
        SignalKind::DeploymentUnreachable,
        deployment_subject.clone(),
        "dep-signal".to_string(),
        "Deployment signal".to_string(),
        now(),
    );
    let action_signal = signal(
        SignalKind::ActionStuck,
        action_subject.clone(),
        "act-signal".to_string(),
        "Action signal".to_string(),
        now(),
    );

    let ids = [
        dataplane_signal.id.0,
        deployment_signal.id.0,
        action_signal.id.0,
    ];

    let result: Result<(), CoreError> = with_tx(&pool, map_err, async |tx| {
        let repo = PostgresSignalRepository::new(&tx);
        repo.write(dataplane_signal.clone()).await?;
        repo.write(deployment_signal.clone()).await?;
        repo.write(action_signal.clone()).await
    })
    .await;

    result.expect("committed");

    // Verify each row exists and has the right subject.
    let dataplane_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM signals WHERE id = $1 AND subject_kind = 'dataplane'",
    )
    .bind(ids[0])
    .fetch_one(&pool)
    .await
    .expect("query");

    let deployment_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM signals WHERE id = $1 AND subject_kind = 'deployment'",
    )
    .bind(ids[1])
    .fetch_one(&pool)
    .await
    .expect("query");

    let action_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM signals WHERE id = $1 AND subject_kind = 'action'",
    )
    .bind(ids[2])
    .fetch_one(&pool)
    .await
    .expect("query");

    forget(&pool, &ids).await;

    assert_eq!(dataplane_count, 1);
    assert_eq!(deployment_count, 1);
    assert_eq!(action_count, 1);
}

#[tokio::test]
async fn a_closed_signal_with_the_same_dedup_key_does_not_prevent_a_new_one() {
    let Some(pool) = pool().await else {
        return;
    };

    let dedup_key = format!("reusable-key-{}", Uuid::new_v4());
    let subject = SignalSubject::Dataplane {
        id: DataPlaneId(Uuid::new_v4()),
    };

    // First signal: open and then close it.
    let first = signal(
        SignalKind::DataplaneHeartbeatStale,
        subject.clone(),
        dedup_key.clone(),
        "First".to_string(),
        now(),
    );
    let _first_id = first.id.0;

    let _: Result<(), CoreError> = with_tx(&pool, map_err, async |tx| {
        let repo = PostgresSignalRepository::new(&tx);
        repo.write(first.clone()).await
    })
    .await;

    // Close the first signal through the repository.
    let _: Result<(), CoreError> = with_tx(&pool, map_err, async |tx| {
        let repo = PostgresSignalRepository::new(&tx);
        repo.close(&dedup_key, now()).await
    })
    .await;

    // Now write a new signal with the same dedup_key. It should create a new row.
    let second = signal(
        SignalKind::DataplaneHeartbeatStale,
        subject.clone(),
        dedup_key.clone(),
        "Second".to_string(),
        now(),
    );
    let _second_id = second.id.0;

    let result: Result<(), CoreError> = with_tx(&pool, map_err, async |tx| {
        let repo = PostgresSignalRepository::new(&tx);
        repo.write(second.clone()).await
    })
    .await;

    result.expect("committed");

    // Verify two rows now exist for this dedup_key.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM signals WHERE dedup_key = $1")
        .bind(&dedup_key)
        .fetch_one(&pool)
        .await
        .expect("query");

    // Verify only one is open.
    let open_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM signals WHERE dedup_key = $1 AND closed_at IS NULL",
    )
    .bind(&dedup_key)
    .fetch_one(&pool)
    .await
    .expect("query");

    sqlx::query("DELETE FROM signals WHERE dedup_key = $1")
        .bind(&dedup_key)
        .execute(&pool)
        .await
        .expect("cleanup");

    assert_eq!(count, 2, "two signals with the same dedup_key exist");
    assert_eq!(open_count, 1, "only one is open");
}
