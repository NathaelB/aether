//! What the audit log guarantees on its own: isolation between organisations
//! and stable pagination while new entries arrive. Both are properties of the
//! table and the query, so they are checked against a real Postgres.
//!
//! Runs only when `DATABASE_URL` is set. See `support::pool`.

use aether_domain::{
    CoreError,
    audit::{
        AuditAction, AuditActor, AuditBatch, AuditChange, AuditEntry, AuditEntryId, AuditTarget,
        AuditTargetKind, ports::AuditRepository,
    },
    organisation::OrganisationId,
};
use aether_persistence::with_tx;
use aether_postgres::audit::PostgresAuditRepository;
use chrono::{DateTime, SubsecRound, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

mod support;
use support::pool;

/// Postgres keeps microseconds; `DateTime<Utc>` keeps nanoseconds. A fixture
/// built straight from `Utc::now()` round-trips exactly on macOS, where the
/// two happen to agree often enough to hide the mismatch, and fails on Linux.
fn now() -> DateTime<Utc> {
    Utc::now().trunc_subsecs(6)
}

/// Each test owns an organisation and a user nobody else writes, so the
/// suite stays parallel against a shared database.
struct Fixture {
    tag: String,
    organisation_id: OrganisationId,
    user_id: Uuid,
}

impl Fixture {
    fn new(label: &str) -> Self {
        Self {
            tag: format!("audit-log-{label}-{}", Uuid::new_v4()),
            organisation_id: OrganisationId(Uuid::new_v4()),
            user_id: Uuid::new_v4(),
        }
    }

    fn entry(
        &self,
        actor: AuditActor,
        action: &str,
        change: Option<AuditChange>,
        recorded_at: DateTime<Utc>,
    ) -> AuditEntry {
        AuditEntry::record(
            AuditEntryId(Uuid::new_v4()),
            self.organisation_id,
            actor,
            AuditAction(action.to_string()),
            AuditTarget {
                kind: AuditTargetKind::Organisation,
                id: self.organisation_id.0,
            },
            change,
            recorded_at,
        )
    }
}

fn map_err(e: sqlx::Error) -> CoreError {
    CoreError::DatabaseError {
        message: e.to_string(),
    }
}

async fn clean(pool: &PgPool, fixture: &Fixture) {
    for statement in [
        "DELETE FROM organisations WHERE name = $1",
        "DELETE FROM users WHERE name = $1",
    ] {
        sqlx::query(statement)
            .bind(&fixture.tag)
            .execute(pool)
            .await
            .expect("cleanup");
    }
}

/// Seeds a user and an organisation. Deleting the organisation on cleanup
/// cascades to whatever audit_log rows the test wrote.
async fn seed(pool: &PgPool, fixture: &Fixture) {
    sqlx::query("INSERT INTO users (id, email, name, sub) VALUES ($1, $2, $3, $4)")
        .bind(fixture.user_id)
        .bind(format!("{}@audit-log.test", fixture.user_id))
        .bind(&fixture.tag)
        .bind(fixture.user_id.to_string())
        .execute(pool)
        .await
        .expect("user seeded");

    sqlx::query(
        "INSERT INTO organisations \
         (id, name, slug, owner_id, status, plan, max_instances, max_users, \
          max_storage_gb, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, 'active', 'free', 5, 5, 5, now(), now())",
    )
    .bind(fixture.organisation_id.0)
    .bind(&fixture.tag)
    .bind(fixture.organisation_id.0.to_string())
    .bind(fixture.user_id)
    .execute(pool)
    .await
    .expect("organisation seeded");
}

#[tokio::test]
async fn an_entry_survives_a_round_trip() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new("round-trip");
    clean(&pool, &fixture).await;
    seed(&pool, &fixture).await;

    let change = AuditChange::new(json!({"minutes": 30}), json!({"minutes": 60}))
        .expect("an innocuous change");
    let written = fixture.entry(
        AuditActor::User {
            user_id: fixture.user_id,
        },
        "deployment.maintenance_window.updated",
        Some(change.clone()),
        now(),
    );

    let result: Result<AuditBatch, CoreError> = with_tx(&pool, map_err, async |tx| {
        let audit = PostgresAuditRepository::new(&tx);
        audit.append(written.clone()).await?;
        audit
            .list_for_organisation(fixture.organisation_id, None, 10)
            .await
    })
    .await;

    let batch = result.expect("committed");
    clean(&pool, &fixture).await;

    assert_eq!(batch.entries.len(), 1);
    let found = &batch.entries[0];
    assert_eq!(found.id, written.id);
    assert_eq!(
        found.actor,
        AuditActor::User {
            user_id: fixture.user_id
        }
    );
    assert_eq!(
        found.action,
        AuditAction("deployment.maintenance_window.updated".to_string())
    );
    assert_eq!(found.recorded_at, written.recorded_at);

    let found_change = found.change.as_ref().expect("the change was stored");
    assert_eq!(found_change.before(), change.before());
    assert_eq!(found_change.after(), change.after());
}

#[tokio::test]
async fn an_entry_with_no_change_round_trips_without_one() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new("no-change");
    clean(&pool, &fixture).await;
    seed(&pool, &fixture).await;

    let written = fixture.entry(AuditActor::System, "organisation.plan.changed", None, now());

    let result: Result<AuditBatch, CoreError> = with_tx(&pool, map_err, async |tx| {
        let audit = PostgresAuditRepository::new(&tx);
        audit.append(written.clone()).await?;
        audit
            .list_for_organisation(fixture.organisation_id, None, 10)
            .await
    })
    .await;

    let batch = result.expect("committed");
    clean(&pool, &fixture).await;

    assert_eq!(batch.entries.len(), 1);
    assert!(batch.entries[0].change.is_none());
}

/// The acceptance criterion #120 exists for: reading one organisation's trail
/// must never surface a row that belongs to another.
#[tokio::test]
async fn an_organisation_never_sees_another_ones_entries() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture_a = Fixture::new("isolation-a");
    let fixture_b = Fixture::new("isolation-b");
    clean(&pool, &fixture_a).await;
    clean(&pool, &fixture_b).await;
    seed(&pool, &fixture_a).await;
    seed(&pool, &fixture_b).await;

    let entry_a = fixture_a.entry(AuditActor::System, "organisation.plan.changed", None, now());
    let entry_b = fixture_b.entry(AuditActor::System, "organisation.plan.changed", None, now());

    let result: Result<AuditBatch, CoreError> = with_tx(&pool, map_err, async |tx| {
        let audit = PostgresAuditRepository::new(&tx);
        audit.append(entry_a.clone()).await?;
        audit.append(entry_b.clone()).await?;
        audit
            .list_for_organisation(fixture_a.organisation_id, None, 50)
            .await
    })
    .await;

    let batch = result.expect("committed");
    clean(&pool, &fixture_a).await;
    clean(&pool, &fixture_b).await;

    let ids: Vec<AuditEntryId> = batch.entries.iter().map(|entry| entry.id).collect();
    assert!(
        ids.contains(&entry_a.id),
        "the organisation's own entry is missing"
    );
    assert!(
        !ids.contains(&entry_b.id),
        "an organisation must never see another one's entries"
    );
}

/// The acceptance criterion #120 exists for: an `OFFSET` page shifts under a
/// reader when a row is inserted ahead of it. A keyset cursor must not.
#[tokio::test]
async fn paging_stays_stable_while_a_new_entry_is_recorded() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new("paging");
    clean(&pool, &fixture).await;
    seed(&pool, &fixture).await;

    let base = now();
    let oldest = fixture.entry(AuditActor::System, "a", None, base);
    let middle = fixture.entry(
        AuditActor::System,
        "b",
        None,
        base + chrono::Duration::seconds(1),
    );
    let newest = fixture.entry(
        AuditActor::System,
        "c",
        None,
        base + chrono::Duration::seconds(2),
    );

    let first_page: AuditBatch = with_tx(&pool, map_err, async |tx| {
        let audit = PostgresAuditRepository::new(&tx);
        audit.append(oldest.clone()).await?;
        audit.append(middle.clone()).await?;
        audit.append(newest.clone()).await?;
        audit
            .list_for_organisation(fixture.organisation_id, None, 2)
            .await
    })
    .await
    .expect("committed");

    assert_eq!(
        first_page
            .entries
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        vec![newest.id, middle.id],
        "newest first"
    );
    let cursor = first_page.next_cursor.clone().expect("more entries remain");

    // Recorded after the first page was read, ahead of everything already
    // fetched: this is the row an `OFFSET` page would have let slip in front
    // of the second page, duplicating `middle` or skipping `oldest`.
    let inserted_while_paging = fixture.entry(
        AuditActor::System,
        "d",
        None,
        base + chrono::Duration::seconds(3),
    );

    let second_page: AuditBatch = with_tx(&pool, map_err, async |tx| {
        let audit = PostgresAuditRepository::new(&tx);
        audit.append(inserted_while_paging.clone()).await?;
        audit
            .list_for_organisation(fixture.organisation_id, Some(cursor), 10)
            .await
    })
    .await
    .expect("committed");

    clean(&pool, &fixture).await;

    let ids: Vec<AuditEntryId> = second_page.entries.iter().map(|entry| entry.id).collect();
    assert_eq!(
        ids,
        vec![oldest.id],
        "the second page must hold exactly what was left, unaffected by the new entry"
    );
    assert!(!ids.contains(&inserted_while_paging.id));
}

#[tokio::test]
async fn an_api_actor_round_trips_its_client_id() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new("api-actor");
    clean(&pool, &fixture).await;
    seed(&pool, &fixture).await;

    let written = fixture.entry(
        AuditActor::Api {
            client_id: "herald-service".to_string(),
        },
        "deployment.upgrade.accepted",
        None,
        now(),
    );

    let result: Result<AuditBatch, CoreError> = with_tx(&pool, map_err, async |tx| {
        let audit = PostgresAuditRepository::new(&tx);
        audit.append(written.clone()).await?;
        audit
            .list_for_organisation(fixture.organisation_id, None, 10)
            .await
    })
    .await;

    let batch = result.expect("committed");
    clean(&pool, &fixture).await;

    assert_eq!(
        batch.entries[0].actor,
        AuditActor::Api {
            client_id: "herald-service".to_string()
        }
    );
}
