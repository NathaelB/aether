use aether_domain::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        ports::DataPlaneRepository,
        value_objects::{Capacity, DataPlaneAllocation, DataPlaneId, DeploymentResources, Region},
    },
    deployments::{
        Deployment, DeploymentId, DeploymentKind, DeploymentName, DeploymentStatus,
        ports::DeploymentRepository,
    },
    organisation::OrganisationId,
    upgrades::{
        path::UpgradePath,
        ports::AcceptedUpgrade,
        run::{UpgradeRun, UpgradeRunId, UpgradeRunOutcome, UpgradeTrigger},
        run_ports::UpgradeRunRepository,
    },
    user::UserId,
    version::{Version, VersionChange},
};
use aether_persistence::with_tx;
use aether_postgres::{
    dataplane::PostgresDataPlaneRepository, deployments::PostgresDeploymentRepository,
    upgrades::PostgresUpgradeRunRepository,
};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// An instant Postgres can hold exactly.
///
/// `timestamptz` keeps microseconds; `DateTime<Utc>` carries nanoseconds. A
/// round trip therefore loses the last three digits, and comparing what was
/// written against what came back fails on whichever machine happens to
/// produce a clock reading that is not a whole microsecond. That is a real
/// property of the column, not a test artifact, so the fixtures use instants
/// the column can represent rather than the assertions being loosened.
fn now() -> chrono::DateTime<Utc> {
    use chrono::SubsecRound;
    Utc::now().trunc_subsecs(6)
}

mod support;
use support::pool;

/// Each test owns an organisation, a data plane and a deployment nobody else
/// writes, so the suite stays parallel against a shared database.
struct Fixture {
    tag: String,
    organisation_id: OrganisationId,
    deployment_id: DeploymentId,
    user_id: UserId,
}

impl Fixture {
    fn new(label: &str) -> Self {
        Self {
            tag: format!("upgrade-run-{label}-{}", Uuid::new_v4()),
            organisation_id: OrganisationId(Uuid::new_v4()),
            deployment_id: DeploymentId(Uuid::new_v4()),
            user_id: UserId(Uuid::new_v4()),
        }
    }

    fn manual(&self) -> UpgradeTrigger {
        UpgradeTrigger::Manual { by: self.user_id }
    }
}

fn map_err(e: sqlx::Error) -> CoreError {
    CoreError::DatabaseError {
        message: e.to_string(),
    }
}

async fn clean(pool: &PgPool, fixture: &Fixture) {
    for statement in [
        "DELETE FROM deployments WHERE dataplane_id IN (SELECT id FROM data_planes WHERE region = $1)",
        "DELETE FROM data_planes WHERE region = $1",
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

/// Seeds a user, an organisation, a data plane and a deployment the runs in
/// this fixture attach to. Deleting the deployment on cleanup cascades to
/// whatever upgrade_runs rows the test wrote.
async fn seed(pool: &PgPool, fixture: &Fixture) {
    let result: Result<(), CoreError> = with_tx(pool, map_err, async |tx| {
        let dataplanes = PostgresDataPlaneRepository::new(&tx);
        let deployments = PostgresDeploymentRepository::new(&tx);

        {
            let mut guard = tx.lock().await;
            sqlx::query("INSERT INTO users (id, email, name, sub) VALUES ($1, $2, $3, $4)")
                .bind(fixture.user_id.0)
                .bind(format!("{}@upgrade-run.test", fixture.user_id.0))
                .bind(&fixture.tag)
                .bind(fixture.user_id.0.to_string())
                .execute(&mut ***guard)
                .await
                .map_err(map_err)?;

            sqlx::query(
                "INSERT INTO organisations \
                 (id, name, slug, owner_id, status, plan, max_instances, max_users, \
                  max_storage_gb, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, 'active', 'free', 5, 5, 5, now(), now())",
            )
            .bind(fixture.organisation_id.0)
            .bind(&fixture.tag)
            .bind(fixture.organisation_id.0.to_string())
            .bind(fixture.user_id.0)
            .execute(&mut ***guard)
            .await
            .map_err(map_err)?;
        }

        let dataplane = DataPlane::new(
            DataPlaneAllocation::Shared,
            Region::new(&fixture.tag),
            Capacity::new(8_000, 16_384, 200).expect("non-zero capacity"),
        );
        dataplanes.save(&dataplane).await?;

        let at = now();
        deployments
            .insert(Deployment {
                id: fixture.deployment_id,
                organisation_id: fixture.organisation_id,
                dataplane_id: dataplane.id,
                name: DeploymentName("auth".to_string()),
                kind: DeploymentKind::Ferriskey,
                version: Version::new(26, 0, 0),
                status: DeploymentStatus::Successful,
                namespace: fixture.tag.clone(),
                resources: DeploymentResources::DEFAULT,
                created_by: fixture.user_id,
                created_at: at,
                updated_at: at,
                deployed_at: None,
                deleted_at: None,
                auto_upgrade: Default::default(),
                maintenance_window: None,
                network_access: aether_domain::deployments::network::NetworkAccess::Open,
            })
            .await?;

        Ok(())
    })
    .await;

    result.expect("fixture setup committed");
}

/// A run for `fixture`'s deployment, unresolved, starting at `started_at`.
fn run(
    fixture: &Fixture,
    from: Version,
    to: Version,
    trigger: UpgradeTrigger,
    started_at: DateTime<Utc>,
) -> UpgradeRun {
    let accepted = AcceptedUpgrade {
        deployment: Deployment {
            id: fixture.deployment_id,
            organisation_id: fixture.organisation_id,
            dataplane_id: DataPlaneId(Uuid::new_v4()),
            name: DeploymentName("auth".to_string()),
            kind: DeploymentKind::Ferriskey,
            version: from.clone(),
            status: DeploymentStatus::Upgrading,
            namespace: fixture.tag.clone(),
            resources: DeploymentResources::DEFAULT,
            created_by: fixture.user_id,
            created_at: started_at,
            updated_at: started_at,
            deployed_at: None,
            deleted_at: None,
            auto_upgrade: Default::default(),
            maintenance_window: None,
            network_access: aether_domain::deployments::network::NetworkAccess::Open,
        },
        change: from.change_to(&to).expect("a forward step"),
        path: UpgradePath::direct(to),
    };

    UpgradeRun::start(UpgradeRunId(Uuid::new_v4()), &accepted, trigger, started_at)
}

#[tokio::test]
async fn an_upgrade_run_survives_a_round_trip() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new("round-trip");
    clean(&pool, &fixture).await;
    seed(&pool, &fixture).await;

    let written = run(
        &fixture,
        Version::new(26, 0, 0),
        Version::new(26, 0, 1),
        fixture.manual(),
        now(),
    );

    let result: Result<Option<UpgradeRun>, CoreError> = with_tx(&pool, map_err, async |tx| {
        let runs = PostgresUpgradeRunRepository::new(&tx);
        runs.insert(written.clone()).await?;
        runs.get(written.id).await
    })
    .await;

    let found = result.expect("committed").expect("the run is there");
    clean(&pool, &fixture).await;

    assert_eq!(found.deployment_id, fixture.deployment_id);
    assert_eq!(found.from_version, Version::new(26, 0, 0));
    assert_eq!(found.to_version, Version::new(26, 0, 1));
    assert_eq!(found.change, VersionChange::Patch);
    assert_eq!(found.trigger, fixture.manual());
    assert_eq!(found.outcome, None, "a fresh run has not concluded");
    assert_eq!(found.ended_at, None);
}

#[tokio::test]
async fn a_scheduled_run_round_trips_without_naming_anyone() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new("scheduled");
    clean(&pool, &fixture).await;
    seed(&pool, &fixture).await;

    let written = run(
        &fixture,
        Version::new(26, 0, 0),
        Version::new(26, 1, 0),
        UpgradeTrigger::Scheduled,
        now(),
    );

    let result: Result<Option<UpgradeRun>, CoreError> = with_tx(&pool, map_err, async |tx| {
        let runs = PostgresUpgradeRunRepository::new(&tx);
        runs.insert(written.clone()).await?;
        runs.get(written.id).await
    })
    .await;

    let found = result.expect("committed").expect("the run is there");
    clean(&pool, &fixture).await;

    assert_eq!(found.trigger, UpgradeTrigger::Scheduled);
    assert_eq!(found.change, VersionChange::Minor);
}

#[tokio::test]
async fn concluding_a_run_writes_back_the_outcome_and_when_it_ended() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new("conclude");
    clean(&pool, &fixture).await;
    seed(&pool, &fixture).await;

    let mut written = run(
        &fixture,
        Version::new(26, 0, 0),
        Version::new(26, 0, 1),
        fixture.manual(),
        now(),
    );
    let ended_at = written.started_at + chrono::Duration::minutes(4);

    let result: Result<Option<UpgradeRun>, CoreError> = with_tx(&pool, map_err, async |tx| {
        let runs = PostgresUpgradeRunRepository::new(&tx);
        runs.insert(written.clone()).await?;

        written
            .succeed(ended_at)
            .expect("an in-progress run may settle");
        runs.update(&written).await?;

        runs.get(written.id).await
    })
    .await;

    let found = result.expect("committed").expect("the run is there");
    clean(&pool, &fixture).await;

    assert_eq!(found.outcome, Some(UpgradeRunOutcome::Succeeded));
    assert_eq!(found.ended_at, Some(ended_at));
}

/// The acceptance criterion this whole table exists for: a deployment's
/// status and version answer only for the most recent attempt, so a failure
/// is only ever visible here.
#[tokio::test]
async fn a_failed_attempt_survives_the_one_that_replaced_it() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new("survives");
    clean(&pool, &fixture).await;
    seed(&pool, &fixture).await;

    let mut first = run(
        &fixture,
        Version::new(26, 0, 0),
        Version::new(26, 0, 1),
        fixture.manual(),
        now(),
    );
    let second = run(
        &fixture,
        Version::new(26, 0, 0),
        Version::new(26, 0, 2),
        fixture.manual(),
        now() + chrono::Duration::minutes(30),
    );

    let result: Result<(Option<UpgradeRun>, Option<UpgradeRun>), CoreError> =
        with_tx(&pool, map_err, async |tx| {
            let runs = PostgresUpgradeRunRepository::new(&tx);

            runs.insert(first.clone()).await?;
            first
                .fail("the operator reported a crash loop", now())
                .expect("an in-progress run may fail");
            runs.update(&first).await?;

            runs.insert(second.clone()).await?;

            let reloaded_first = runs.get(first.id).await?;
            let reloaded_second = runs.get(second.id).await?;
            Ok((reloaded_first, reloaded_second))
        })
        .await;

    let (reloaded_first, reloaded_second) = result.expect("committed");
    clean(&pool, &fixture).await;

    let reloaded_first = reloaded_first.expect("the failed run is still there");
    assert_eq!(reloaded_first.outcome, Some(UpgradeRunOutcome::Failed));
    assert_eq!(
        reloaded_first.detail.as_deref(),
        Some("the operator reported a crash loop")
    );

    let reloaded_second = reloaded_second.expect("the second run is there");
    assert_eq!(
        reloaded_second.outcome, None,
        "the second run is untouched by the first's outcome"
    );
}

/// Listing orders on `started_at` directly: unlike a version, a timestamp
/// compares correctly in SQL, so there is no reason to sort it in Rust.
#[tokio::test]
async fn a_deployments_history_reads_newest_first() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new("history");
    clean(&pool, &fixture).await;
    seed(&pool, &fixture).await;

    let base = now();
    let oldest = run(
        &fixture,
        Version::new(26, 0, 0),
        Version::new(26, 0, 1),
        fixture.manual(),
        base,
    );
    let middle = run(
        &fixture,
        Version::new(26, 0, 1),
        Version::new(26, 1, 0),
        UpgradeTrigger::Scheduled,
        base + chrono::Duration::hours(1),
    );
    let newest = run(
        &fixture,
        Version::new(26, 1, 0),
        Version::new(27, 0, 0),
        fixture.manual(),
        base + chrono::Duration::hours(2),
    );

    let result: Result<Vec<UpgradeRun>, CoreError> = with_tx(&pool, map_err, async |tx| {
        let runs = PostgresUpgradeRunRepository::new(&tx);
        // Inserted out of chronological order on purpose: the ordering has to
        // come from `started_at`, not from insertion order.
        runs.insert(middle.clone()).await?;
        runs.insert(newest.clone()).await?;
        runs.insert(oldest.clone()).await?;

        runs.list_for_deployment(fixture.deployment_id).await
    })
    .await;

    let listed = result.expect("committed");
    clean(&pool, &fixture).await;

    let ids: Vec<UpgradeRunId> = listed.iter().map(|run| run.id).collect();
    assert_eq!(ids, vec![newest.id, middle.id, oldest.id]);

    let triggers: Vec<&UpgradeTrigger> = listed.iter().map(|run| &run.trigger).collect();
    assert_eq!(
        triggers,
        vec![
            &fixture.manual(),
            &UpgradeTrigger::Scheduled,
            &fixture.manual()
        ],
        "each run keeps the trigger it started with"
    );
}

#[tokio::test]
async fn concluding_a_run_that_is_not_there_is_refused() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new("absent");
    clean(&pool, &fixture).await;
    seed(&pool, &fixture).await;

    let mut absent = run(
        &fixture,
        Version::new(26, 0, 0),
        Version::new(26, 0, 1),
        fixture.manual(),
        now(),
    );
    absent.succeed(now()).expect("settles in memory only");

    let result: Result<CoreError, CoreError> = with_tx(&pool, map_err, async |tx| {
        let runs = PostgresUpgradeRunRepository::new(&tx);
        Ok(runs.update(&absent).await.expect_err("nothing to update"))
    })
    .await;

    let error = result.expect("committed");
    clean(&pool, &fixture).await;

    assert!(
        matches!(error, CoreError::UpgradeRunNotFound { .. }),
        "got {error:?}"
    );
}
