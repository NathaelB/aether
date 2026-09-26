use aether_domain::{
    CoreError,
    dataplane::value_objects::DataPlaneId,
    deployments::DeploymentId,
    signals::{
        Signal, SignalId, SignalKind, SignalSubject,
        ports::{SignalListPage, SignalRepository},
    },
};
use aether_persistence::with_tx;
use aether_postgres::signals::PostgresSignalRepository;
use chrono::{DateTime, SubsecRound, Utc};
use sqlx::PgPool;
use uuid::Uuid;

mod support;
use support::pool;

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

    let first = signal(
        SignalKind::DataplaneHeartbeatStale,
        subject.clone(),
        dedup_key.clone(),
        "First message".to_string(),
        now(),
    );
    let _first_id = first.id.0;

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

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM signals WHERE dedup_key = $1 AND closed_at IS NULL",
    )
    .bind(&dedup_key)
    .fetch_one(&pool)
    .await
    .expect("query");

    let message: String = sqlx::query_scalar(
        "SELECT message FROM signals WHERE dedup_key = $1 AND closed_at IS NULL",
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

    let _: Result<(), CoreError> = with_tx(&pool, map_err, async |tx| {
        let repo = PostgresSignalRepository::new(&tx);
        repo.close(&dedup_key, now()).await
    })
    .await;

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

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM signals WHERE dedup_key = $1")
        .bind(&dedup_key)
        .fetch_one(&pool)
        .await
        .expect("query");

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

#[tokio::test]
async fn list_open_excludes_closed_signals() {
    let Some(pool) = pool().await else {
        return;
    };

    let dedup_key = format!("list-open-closed-{}", Uuid::new_v4());
    let subject = SignalSubject::Dataplane {
        id: DataPlaneId(Uuid::new_v4()),
    };
    let written = signal(
        SignalKind::DataplaneHeartbeatStale,
        subject,
        dedup_key.clone(),
        "Will be closed".to_string(),
        now(),
    );
    let id = written.id.0;

    with_tx(&pool, map_err, async |tx| {
        let repo = PostgresSignalRepository::new(&tx);
        repo.write(written.clone()).await?;
        repo.close(&dedup_key, now()).await
    })
    .await
    .expect("committed");

    let page: SignalListPage = with_tx(&pool, map_err, async |tx| {
        let repo = PostgresSignalRepository::new(&tx);
        repo.list_open(None, None, 1000, None).await
    })
    .await
    .expect("query");

    forget(&pool, &[id]).await;

    assert!(
        page.signals.iter().all(|s| s.id.0 != id),
        "a closed signal must not appear in an open-signals listing"
    );
}

#[tokio::test]
async fn list_open_filters_by_kind_and_subject() {
    let Some(pool) = pool().await else {
        return;
    };

    let target_subject = SignalSubject::Deployment {
        id: DeploymentId(Uuid::new_v4()),
    };
    let target = signal(
        SignalKind::BackupMissing,
        target_subject.clone(),
        format!("list-open-target-{}", Uuid::new_v4()),
        "Target".to_string(),
        now(),
    );
    let other = signal(
        SignalKind::DrillOverdue,
        SignalSubject::Dataplane {
            id: DataPlaneId(Uuid::new_v4()),
        },
        format!("list-open-other-{}", Uuid::new_v4()),
        "Other".to_string(),
        now(),
    );
    let ids = [target.id.0, other.id.0];

    with_tx(&pool, map_err, async |tx| {
        let repo = PostgresSignalRepository::new(&tx);
        repo.write(target.clone()).await?;
        repo.write(other.clone()).await
    })
    .await
    .expect("committed");

    let by_kind: SignalListPage = with_tx(&pool, map_err, async |tx| {
        let repo = PostgresSignalRepository::new(&tx);
        repo.list_open(Some(SignalKind::BackupMissing), None, 1000, None)
            .await
    })
    .await
    .expect("query");

    let by_subject: SignalListPage = with_tx(&pool, map_err, async |tx| {
        let repo = PostgresSignalRepository::new(&tx);
        repo.list_open(None, Some(target_subject), 1000, None).await
    })
    .await
    .expect("query");

    forget(&pool, &ids).await;

    assert!(
        by_kind.signals.iter().any(|s| s.id.0 == target.id.0),
        "the kind filter must include the matching signal"
    );
    assert!(
        by_kind.signals.iter().all(|s| s.id.0 != other.id.0),
        "the kind filter must exclude a signal of a different kind"
    );
    assert!(
        by_subject.signals.iter().any(|s| s.id.0 == target.id.0),
        "the subject filter must include the matching signal"
    );
    assert!(
        by_subject.signals.iter().all(|s| s.id.0 != other.id.0),
        "the subject filter must exclude a signal on a different subject"
    );
}

#[tokio::test]
async fn list_open_pagination_does_not_lose_signals_sharing_the_same_opened_at() {
    let Some(pool) = pool().await else {
        return;
    };

    let tick = now();
    let written: Vec<Signal> = (0..5)
        .map(|_| {
            signal(
                SignalKind::DataplaneHeartbeatStale,
                SignalSubject::Dataplane {
                    id: DataPlaneId(Uuid::new_v4()),
                },
                format!("list-open-tie-{}", Uuid::new_v4()),
                "Same tick".to_string(),
                tick,
            )
        })
        .collect();
    let ids: Vec<Uuid> = written.iter().map(|s| s.id.0).collect();

    with_tx(&pool, map_err, async |tx| {
        let repo = PostgresSignalRepository::new(&tx);
        for signal in &written {
            repo.write(signal.clone()).await?;
        }
        Ok(())
    })
    .await
    .expect("committed");

    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page: SignalListPage = with_tx(&pool, map_err, async |tx| {
            let repo = PostgresSignalRepository::new(&tx);
            repo.list_open(None, None, 2, cursor.clone()).await
        })
        .await
        .expect("query");

        seen.extend(
            page.signals
                .iter()
                .filter(|s| ids.contains(&s.id.0))
                .map(|s| s.id.0),
        );

        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }

    forget(&pool, &ids).await;

    seen.sort();
    let mut expected = ids.clone();
    expected.sort();
    assert_eq!(
        seen, expected,
        "every signal opened in the same tick must be returned exactly once across pages"
    );
}
