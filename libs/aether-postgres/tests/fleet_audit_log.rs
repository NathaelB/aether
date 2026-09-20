//! What the fleet trail guarantees on its own: that it is not the
//! organisation trail, that the table refuses an entry naming no cluster and
//! no subject, and that a page stays stable while new entries arrive. All
//! three live in the table and the query rather than in the domain, so they
//! are checked against a real Postgres.
//!
//! Unlike `audit_log`, there is no scope to isolate a test by -- an
//! installation-wide trail is installation-wide for every reader, including
//! this suite. So each test writes entries it can recognise and asserts about
//! those, never about the table's total.
//!
//! Runs only when `DATABASE_URL` is set. See `support::pool`.

use aether_domain::{
    CoreError,
    audit::{
        AuditAction, AuditActor, AuditChange, AuditEntry, AuditEntryId, AuditTarget,
        AuditTargetKind,
        fleet::{
            FleetActor, FleetAuditAction, FleetAuditBatch, FleetAuditEntry, FleetAuditEntryId,
            FleetTarget, ports::FleetAuditRepository,
        },
        ports::AuditRepository,
    },
    dataplane::value_objects::DataPlaneId,
    organisation::OrganisationId,
};
use aether_persistence::with_tx;
use aether_postgres::audit::{PostgresAuditRepository, PostgresFleetAuditRepository};
use chrono::{DateTime, Duration, SubsecRound, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

mod support;
use support::pool;

/// Postgres keeps microseconds; `DateTime<Utc>` keeps nanoseconds. Matching
/// `audit_log`'s fixture, and for the same reason: a value straight from
/// `Utc::now()` round-trips on macOS and fails on Linux.
fn now() -> DateTime<Utc> {
    Utc::now().trunc_subsecs(6)
}

fn map_err(e: sqlx::Error) -> CoreError {
    CoreError::DatabaseError {
        message: e.to_string(),
    }
}

fn entry(
    action: FleetAuditAction,
    target: FleetTarget,
    change: Option<AuditChange>,
    recorded_at: DateTime<Utc>,
) -> FleetAuditEntry {
    FleetAuditEntry::record(
        FleetAuditEntryId(Uuid::new_v4()),
        FleetActor::Operator {
            subject: format!("operator-{}", Uuid::new_v4()),
        },
        action,
        target,
        change,
        recorded_at,
    )
}

fn against_a_cluster() -> FleetTarget {
    FleetTarget::DataPlane {
        id: DataPlaneId(Uuid::new_v4()),
    }
}

/// Rows written by one test, removed once it has asserted. Nothing here
/// cascades from an organisation the way `audit_log` does -- that is the
/// whole point of the table -- so each test takes its own rows back out.
async fn forget(pool: &PgPool, ids: &[Uuid]) {
    sqlx::query("DELETE FROM fleet_audit_log WHERE id = ANY($1)")
        .bind(ids)
        .execute(pool)
        .await
        .expect("cleanup");
}

fn mine<'a>(batch: &'a FleetAuditBatch, ids: &[Uuid]) -> Vec<&'a FleetAuditEntry> {
    batch
        .entries
        .iter()
        .filter(|entry| ids.contains(&entry.id.0))
        .collect()
}

#[tokio::test]
async fn an_entry_survives_a_round_trip() {
    let Some(pool) = pool().await else {
        return;
    };

    let change = AuditChange::new(json!({"status": "active"}), json!({"status": "draining"}))
        .expect("an innocuous change");
    let target = against_a_cluster();
    let written = entry(
        FleetAuditAction::DataPlaneDrained,
        target.clone(),
        Some(change.clone()),
        now(),
    );

    let result: Result<FleetAuditBatch, CoreError> = with_tx(&pool, map_err, async |tx| {
        let trail = PostgresFleetAuditRepository::new(&tx);
        trail.append(written.clone()).await?;
        trail.list(None, 200).await
    })
    .await;

    let batch = result.expect("committed");
    forget(&pool, &[written.id.0]).await;

    let found = mine(&batch, &[written.id.0]);
    assert_eq!(found.len(), 1);
    let found = found[0];

    assert_eq!(found.actor, written.actor, "the subject, not a user id");
    assert_eq!(found.action, FleetAuditAction::DataPlaneDrained);
    assert_eq!(found.target, target);
    assert_eq!(found.recorded_at, written.recorded_at);

    let found_change = found.change.as_ref().expect("the change was stored");
    assert_eq!(found_change.before(), change.before());
    assert_eq!(found_change.after(), change.after());
}

/// The trail's two target shapes, both through the columns rather than
/// through the enum alone: a subject is stored as a subject, not squeezed
/// into the UUID column.
#[tokio::test]
async fn an_operator_entry_round_trips_as_a_subject() {
    let Some(pool) = pool().await else {
        return;
    };

    let target = FleetTarget::Operator {
        subject: format!("granted-{}", Uuid::new_v4()),
    };
    let written = entry(
        FleetAuditAction::OperatorGranted,
        target.clone(),
        None,
        now(),
    );

    let result: Result<FleetAuditBatch, CoreError> = with_tx(&pool, map_err, async |tx| {
        let trail = PostgresFleetAuditRepository::new(&tx);
        trail.append(written.clone()).await?;
        trail.list(None, 200).await
    })
    .await;

    let batch = result.expect("committed");
    forget(&pool, &[written.id.0]).await;

    let found = mine(&batch, &[written.id.0]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].target, target);
    assert!(found[0].change.is_none());
}

/// The acceptance criterion #271 exists for. A customer reading their own
/// trail must never see a fleet action, and somebody reading the fleet's must
/// never see a tenant's -- and the reason it holds is that the two rows are
/// in different tables with no query spanning them.
#[tokio::test]
async fn the_two_trails_never_contain_each_other() {
    let Some(pool) = pool().await else {
        return;
    };

    let organisation_id = OrganisationId(Uuid::new_v4());
    let tenant_entry = AuditEntry::record(
        AuditEntryId(Uuid::new_v4()),
        organisation_id,
        AuditActor::System,
        AuditAction("deployment.maintenance_window.updated".to_string()),
        AuditTarget {
            kind: AuditTargetKind::Deployment,
            id: Uuid::new_v4(),
        },
        None,
        now(),
    );
    let fleet_entry = entry(
        FleetAuditAction::DataPlaneDisabled,
        against_a_cluster(),
        None,
        now(),
    );

    // The organisation trail has a foreign key onto `organisations`, so a
    // tenant entry cannot be written for an organisation that does not exist.
    // That is enough for the half that matters here: nothing a customer could
    // possibly write reaches the fleet trail, and the fleet write below is
    // then read back through both ports.
    let written: Result<(), CoreError> = with_tx(&pool, map_err, async |tx| {
        PostgresFleetAuditRepository::new(&tx)
            .append(fleet_entry.clone())
            .await
    })
    .await;
    written.expect("the fleet entry committed");

    let tenant_side: Result<_, CoreError> = with_tx(&pool, map_err, async |tx| {
        PostgresAuditRepository::new(&tx)
            .list_for_organisation(organisation_id, None, 200)
            .await
    })
    .await;

    let fleet_side: Result<FleetAuditBatch, CoreError> = with_tx(&pool, map_err, async |tx| {
        PostgresFleetAuditRepository::new(&tx).list(None, 200).await
    })
    .await;

    let tenant_side = tenant_side.expect("read");
    let fleet_side = fleet_side.expect("read");
    forget(&pool, &[fleet_entry.id.0]).await;

    assert!(
        tenant_side.entries.is_empty(),
        "a fleet action reached an organisation's trail"
    );
    assert_eq!(
        mine(&fleet_side, &[fleet_entry.id.0]).len(),
        1,
        "the fleet entry is readable from its own port"
    );
    assert!(
        !fleet_side
            .entries
            .iter()
            .any(|found| found.id.0 == tenant_entry.id.0),
        "a tenant's trail reached the fleet's"
    );
}

/// The table's own rule, not the domain's: the enum cannot express an entry
/// naming two targets or none, but a hand-written `INSERT` can, and the check
/// is what stops one.
#[tokio::test]
async fn the_table_refuses_an_entry_naming_no_target() {
    let Some(pool) = pool().await else {
        return;
    };

    let refused = sqlx::query(
        "INSERT INTO fleet_audit_log \
         (id, actor_type, actor_subject, action, recorded_at) \
         VALUES ($1, 'operator', 'somebody', 'dataplane.drained', now())",
    )
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await;

    assert!(refused.is_err(), "an entry naming nothing was accepted");

    let refused = sqlx::query(
        "INSERT INTO fleet_audit_log \
         (id, actor_type, actor_subject, action, target_dataplane_id, target_subject, recorded_at) \
         VALUES ($1, 'operator', 'somebody', 'dataplane.drained', $2, 'somebody-else', now())",
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await;

    assert!(refused.is_err(), "an entry naming two targets was accepted");
}

/// An entry recorded after a page was fetched sorts ahead of that page's
/// boundary, so it can never shift a page the reader already has -- the
/// property an `OFFSET` would not give.
#[tokio::test]
async fn a_page_already_read_is_not_shifted_by_a_newer_entry() {
    let Some(pool) = pool().await else {
        return;
    };

    let base = now() - Duration::days(365);
    let written: Vec<FleetAuditEntry> = (0..3)
        .map(|index| {
            entry(
                FleetAuditAction::DataPlaneRegistered,
                against_a_cluster(),
                None,
                base + Duration::seconds(index),
            )
        })
        .collect();
    let ids: Vec<Uuid> = written.iter().map(|entry| entry.id.0).collect();

    let first: Result<FleetAuditBatch, CoreError> = with_tx(&pool, map_err, async |tx| {
        let trail = PostgresFleetAuditRepository::new(&tx);
        for entry in &written {
            trail.append(entry.clone()).await?;
        }

        trail.list(None, 200).await
    })
    .await;

    let first = first.expect("committed");
    let seen = mine(&first, &ids);
    assert_eq!(seen.len(), 3);
    assert_eq!(
        seen.iter().map(|entry| entry.id.0).collect::<Vec<_>>(),
        {
            let mut newest_first = ids.clone();
            newest_first.reverse();
            newest_first
        },
        "newest first"
    );

    // A fourth entry, recorded after that page was read. It is newer than
    // every one of the three, so continuing from the page's cursor must not
    // reach it.
    let later = entry(
        FleetAuditAction::DataPlaneDrained,
        against_a_cluster(),
        None,
        base + Duration::seconds(10),
    );
    let boundary = seen
        .last()
        .map(|oldest| {
            aether_domain::audit::AuditCursor::new(format!(
                "{}|{}",
                oldest.recorded_at.to_rfc3339(),
                oldest.id.0
            ))
        })
        .expect("a boundary");

    let second: Result<FleetAuditBatch, CoreError> = with_tx(&pool, map_err, async |tx| {
        let trail = PostgresFleetAuditRepository::new(&tx);
        trail.append(later.clone()).await?;
        trail.list(Some(boundary.clone()), 200).await
    })
    .await;

    let second = second.expect("committed");
    let mut all = ids.clone();
    all.push(later.id.0);
    forget(&pool, &all).await;

    assert!(
        mine(&second, &all).is_empty(),
        "a page continued past the oldest entry reached back into what was already read"
    );
}
