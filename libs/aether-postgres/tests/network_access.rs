//! Who may reach a deployment, against the real column.
//!
//! The unit tests cover the rule. This covers what they cannot: that the
//! domain's shape and the database's shape are the same shape, in both
//! directions, and that a write through the repository actually lands.
//!
//! Runs only when `DATABASE_URL` is set. See `heartbeat_activates.rs`.

use aether_domain::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        ports::DataPlaneRepository,
        value_objects::{Capacity, DataPlaneAllocation, DataPlaneId, DeploymentResources, Region},
    },
    deployments::{
        Deployment, DeploymentId, DeploymentKind, DeploymentName, DeploymentStatus,
        network::{Cidr, NetworkAccess},
        ports::DeploymentRepository,
    },
    organisation::OrganisationId,
    upgrades::policy::{AutoUpgradePolicy, MaintenanceWindow},
    user::UserId,
    version::Version,
};
use aether_persistence::with_tx;
use aether_postgres::{
    dataplane::PostgresDataPlaneRepository, deployments::PostgresDeploymentRepository,
};
use chrono::{Duration, Utc, Weekday};
use uuid::Uuid;

mod support;
use support::pool;

const TEST_REGION: &str = "network-access-test";

fn cidr(raw: &str) -> Cidr {
    raw.parse().expect("a valid range")
}

/// A restricted list written through the repository comes back the same.
#[tokio::test]
async fn an_allow_list_survives_the_round_trip() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let read: Result<NetworkAccess, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let deployments = PostgresDeploymentRepository::new(&tx);
            let mut deployment = seed(&tx).await?;

            deployment.network_access =
                NetworkAccess::from_ranges(vec![cidr("203.0.113.0/24"), cidr("2001:db8::/32")])
                    .expect("restricted")
                    .clone();
            deployments.update(deployment.clone()).await?;

            let back = deployments
                .get_by_id(deployment.id)
                .await?
                .expect("the deployment");

            Ok(back.network_access)
        },
    )
    .await;

    clean_up(&pool).await;

    let read = read.expect("the transaction");
    let ranges: Vec<String> = read.ranges().iter().map(ToString::to_string).collect();

    assert_eq!(ranges, vec!["203.0.113.0/24", "2001:db8::/32"]);
}

/// The default, and the state a column that was never written reads back as.
#[tokio::test]
async fn a_deployment_nobody_restricted_is_open() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let read: Result<NetworkAccess, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let deployments = PostgresDeploymentRepository::new(&tx);
            let deployment = seed(&tx).await?;

            let back = deployments
                .get_by_id(deployment.id)
                .await?
                .expect("the deployment");

            Ok(back.network_access)
        },
    )
    .await;

    clean_up(&pool).await;

    assert_eq!(read.expect("the transaction"), NetworkAccess::Open);
}

/// Going back to open has to clear the column, not leave the previous ranges
/// behind. A restriction that survives being removed is the failure mode this
/// whole slice is about, in the other direction.
#[tokio::test]
async fn going_back_to_open_clears_what_was_there() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let read: Result<NetworkAccess, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let deployments = PostgresDeploymentRepository::new(&tx);
            let mut deployment = seed(&tx).await?;

            deployment.network_access =
                NetworkAccess::from_ranges(vec![cidr("10.0.0.0/8")]).expect("restricted");
            deployments.update(deployment.clone()).await?;

            deployment.network_access = NetworkAccess::Open;
            deployments.update(deployment.clone()).await?;

            let back = deployments
                .get_by_id(deployment.id)
                .await?
                .expect("the deployment");

            Ok(back.network_access)
        },
    )
    .await;

    clean_up(&pool).await;

    assert_eq!(read.expect("the transaction"), NetworkAccess::Open);
}

/// Not about network access at all, and here because this is where it was
/// found: `update` wrote eight columns and claimed to write a Deployment.
/// Setting an upgrade policy or a maintenance window returned the new values
/// to the caller and left the row untouched, so the scheduler read `manual`
/// for ever and no unattended upgrade could fire.
#[tokio::test]
async fn updating_a_deployment_writes_every_field_it_was_given() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let read: Result<Deployment, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let deployments = PostgresDeploymentRepository::new(&tx);
            let mut deployment = seed(&tx).await?;

            deployment.auto_upgrade = AutoUpgradePolicy::PatchAndMinor;
            deployment.maintenance_window = Some(MaintenanceWindow {
                day: Weekday::Sun,
                start: chrono::NaiveTime::from_hms_opt(3, 0, 0).expect("a time"),
                duration: Duration::minutes(120),
                timezone: chrono_tz::Europe::Paris,
            });
            deployments.update(deployment.clone()).await?;

            deployments
                .get_by_id(deployment.id)
                .await?
                .ok_or(CoreError::DeploymentNotFound {
                    id: deployment.id.0,
                })
        },
    )
    .await;

    clean_up(&pool).await;

    let back = read.expect("the transaction");
    assert_eq!(back.auto_upgrade, AutoUpgradePolicy::PatchAndMinor);

    let window = back
        .maintenance_window
        .expect("the window that was written");
    assert_eq!(window.day, Weekday::Sun);
    assert_eq!(window.duration, Duration::minutes(120));
    assert_eq!(window.timezone, chrono_tz::Europe::Paris);
}

/// Removes what a run of this suite wrote.
///
/// `with_tx` commits, so a suite that does not clean up leaves its rows in
/// whatever database it was pointed at. Thirty-five runs of this one had
/// filled a development database with deployments named `network` before
/// anybody noticed.
///
/// Order matters: `deployments.created_by` is NOT NULL, so a user cannot go
/// before the deployments pointing at it.
async fn clean_up(pool: &sqlx::PgPool) {
    for statement in [
        "DELETE FROM actions WHERE deployment_id IN (SELECT id FROM deployments WHERE dataplane_id IN (SELECT id FROM data_planes WHERE region = $1))",
        "DELETE FROM deployments WHERE dataplane_id IN (SELECT id FROM data_planes WHERE region = $1)",
        "DELETE FROM data_planes WHERE region = $1",
        "DELETE FROM organisations WHERE name = $1",
        "DELETE FROM users WHERE name = 'network'",
    ] {
        sqlx::query(statement)
            .bind(TEST_REGION)
            .execute(pool)
            .await
            .expect("cleanup");
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
        sqlx::query("INSERT INTO users (id, email, name, sub) VALUES ($1, $2, 'network', $3)")
            .bind(user_id.0)
            .bind(format!("{}@network.test", user_id.0))
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
        // Named rather than given the id as a name, so the suite can find its
        // own rows again to remove them.
        .bind(TEST_REGION)
        .bind(organisation_id.0.to_string())
        .bind(user_id.0)
        .execute(&mut ***guard)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: e.to_string(),
        })?;
    }

    let now = Utc::now();
    let deployment = Deployment {
        id: DeploymentId(Uuid::new_v4()),
        organisation_id,
        dataplane_id: DataPlaneId(dataplane.id.0),
        name: DeploymentName("network".to_string()),
        kind: DeploymentKind::Ferriskey,
        version: Version::new(0, 5, 0),
        status: DeploymentStatus::Successful,
        namespace: "network-test".to_string(),
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
