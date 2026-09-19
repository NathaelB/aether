//! Two live deployments cannot hold one hostname (#184), checked against a
//! real Postgres rather than a mock.
//!
//! The rule lives in a partial unique index on `(organisation_id,
//! hostname_slug) WHERE deleted_at IS NULL` -- SQL, not a service-level
//! check -- so this is the shape of logic that has to meet the real engine.
//! See `heartbeat_activates.rs` for why these skip without `DATABASE_URL`.

use aether_domain::{
    CoreError,
    dataplane::{
        ports::DataPlaneRepository,
        value_objects::{Capacity, DataPlaneAllocation, DataPlaneStatus, Region},
    },
    deployments::{
        Deployment, DeploymentId, DeploymentKind, DeploymentName, DeploymentStatus,
        ports::DeploymentRepository,
    },
    organisation::OrganisationId,
    user::UserId,
};
use aether_postgres::{
    dataplane::PostgresDataPlaneRepository, deployments::PostgresDeploymentRepository,
};
use chrono::Utc;
use uuid::Uuid;

use aether_persistence::in_scratch_tx;

mod support;
use support::pool;

const TEST_REGION: &str = "deployment-hostname-uniqueness-test";

async fn seed_organisation(
    tx: &aether_persistence::SharedTx<'_>,
    label: &str,
) -> Result<(UserId, OrganisationId), CoreError> {
    let user_id = UserId(Uuid::new_v4());
    let organisation_id = OrganisationId(Uuid::new_v4());

    let mut guard = tx.lock().await;
    sqlx::query("INSERT INTO users (id, email, name, sub) VALUES ($1, $2, 'bound', $3)")
        .bind(user_id.0)
        .bind(format!("{}@bound.test", user_id.0))
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
         VALUES ($1, $2, $3, $4, 'active', 'free', 100, 100, 100, now(), now())",
    )
    .bind(organisation_id.0)
    .bind(label)
    .bind(format!("{label}-{}", organisation_id.0))
    .bind(user_id.0)
    .execute(&mut ***guard)
    .await
    .map_err(|e| CoreError::DatabaseError {
        message: e.to_string(),
    })?;

    Ok((user_id, organisation_id))
}

async fn seed_dataplane(tx: &aether_persistence::SharedTx<'_>) -> Result<Uuid, CoreError> {
    let dataplanes = PostgresDataPlaneRepository::new(tx);

    let mut dataplane = aether_domain::dataplane::entities::DataPlane::new(
        DataPlaneAllocation::Shared,
        Region::new(TEST_REGION),
        Capacity::new(64_000, 131_072, 2_000).expect("non-zero capacity"),
    );
    dataplane.status = DataPlaneStatus::Active;
    dataplanes.save(&dataplane).await?;

    Ok(dataplane.id.0)
}

fn deployment(
    organisation_id: OrganisationId,
    dataplane_id: Uuid,
    created_by: UserId,
    name: &str,
) -> Deployment {
    let at = Utc::now();

    Deployment {
        id: DeploymentId(Uuid::new_v4()),
        organisation_id,
        dataplane_id: aether_domain::dataplane::value_objects::DataPlaneId(dataplane_id),
        name: DeploymentName(name.to_string()),
        kind: DeploymentKind::Ferriskey,
        version: aether_domain::version::Version::new(26, 0, 1),
        status: DeploymentStatus::Successful,
        namespace: format!("hostname-uniqueness-{name}"),
        environment: aether_domain::deployments::environment::Environment::Development,
        offer: None,
        restored_from: None,
        resources: aether_domain::dataplane::value_objects::DeploymentResources::DEFAULT,
        created_by,
        created_at: at,
        updated_at: at,
        deployed_at: None,
        deleted_at: None,
        auto_upgrade: Default::default(),
        maintenance_window: None,
        network_access: aether_domain::deployments::network::NetworkAccess::Open,
    }
}

/// The point of the issue: a second live deployment with the same hostname
/// in the same organisation is refused by the database itself, not just by
/// whatever service happened to check first.
#[tokio::test]
async fn a_second_live_deployment_with_the_same_hostname_is_refused() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let result: Result<bool, CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let dataplane_id = seed_dataplane(&tx).await?;
            let (user_id, organisation_id) = seed_organisation(&tx, "acme").await?;
            let deployments = PostgresDeploymentRepository::new(&tx);

            deployments
                .insert(deployment(
                    organisation_id,
                    dataplane_id,
                    user_id,
                    "Acme Prod",
                ))
                .await?;

            // A different label, the same slug -- exactly the collision
            // #281 already worried about and this closes.
            let collision = deployments
                .insert(deployment(
                    organisation_id,
                    dataplane_id,
                    user_id,
                    "acme prod",
                ))
                .await;

            Ok(collision.is_err())
        },
    )
    .await;

    assert!(
        result.expect("the transaction committed"),
        "a second live deployment with a colliding hostname must be refused"
    );
}

/// Two organisations naming a deployment alike were never going to collide:
/// #281 scopes a deployment's hostname by its own organisation's slug, which
/// is unique by construction.
#[tokio::test]
async fn two_organisations_can_each_use_the_same_hostname() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let result: Result<(), CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let dataplane_id = seed_dataplane(&tx).await?;
            let (mine_user, mine_org) = seed_organisation(&tx, "acme").await?;
            let (theirs_user, theirs_org) = seed_organisation(&tx, "globex").await?;
            let deployments = PostgresDeploymentRepository::new(&tx);

            deployments
                .insert(deployment(mine_org, dataplane_id, mine_user, "prod"))
                .await?;
            deployments
                .insert(deployment(theirs_org, dataplane_id, theirs_user, "prod"))
                .await?;

            Ok(())
        },
    )
    .await;

    result.expect("neither organisation's hostname blocks the other's");
}

/// A deleted deployment's hostname is free to reuse. Without the partial
/// index scoping this to live rows, a customer could never rename or
/// recreate a deployment under a name they had already deleted.
#[tokio::test]
async fn a_deleted_deployments_hostname_can_be_reused() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let result: Result<(), CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let dataplane_id = seed_dataplane(&tx).await?;
            let (user_id, organisation_id) = seed_organisation(&tx, "acme").await?;
            let deployments = PostgresDeploymentRepository::new(&tx);

            let first = deployment(organisation_id, dataplane_id, user_id, "prod");
            deployments.insert(first.clone()).await?;
            deployments.delete(first.id).await?;

            deployments
                .insert(deployment(organisation_id, dataplane_id, user_id, "prod"))
                .await?;

            Ok(())
        },
    )
    .await;

    result.expect("a deleted deployment's hostname does not block reusing it");
}

/// The exact sequence the application's cutover uses: park one row's name
/// under something that collides with nothing, give the other row the
/// freed name, then give the first row what the second gave up. None of the
/// three writes ever puts two live rows on the same hostname at once, so
/// the constraint that proves that never has cause to refuse a legitimate
/// swap.
#[tokio::test]
async fn the_three_step_swap_never_trips_the_constraint() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let result: Result<(String, String), CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let dataplane_id = seed_dataplane(&tx).await?;
            let (user_id, organisation_id) = seed_organisation(&tx, "acme").await?;
            let deployments = PostgresDeploymentRepository::new(&tx);

            let mut source = deployment(organisation_id, dataplane_id, user_id, "acme-prod");
            let mut recovery = deployment(organisation_id, dataplane_id, user_id, "acme-recovery");
            deployments.insert(source.clone()).await?;
            deployments.insert(recovery.clone()).await?;

            let parking = format!("cutover-{}", Uuid::new_v4());

            recovery.name = DeploymentName(parking);
            deployments.update(recovery.clone()).await?;

            source.name = DeploymentName("acme-recovery".to_string());
            deployments.update(source.clone()).await?;

            recovery.name = DeploymentName("acme-prod".to_string());
            deployments.update(recovery.clone()).await?;

            let source_after = deployments
                .get_by_id(source.id)
                .await?
                .expect("still there");
            let recovery_after = deployments
                .get_by_id(recovery.id)
                .await?
                .expect("still there");

            Ok((source_after.name.0, recovery_after.name.0))
        },
    )
    .await;

    let (source_name, recovery_name) =
        result.expect("the swap committed without a constraint violation");
    assert_eq!(source_name, "acme-recovery");
    assert_eq!(recovery_name, "acme-prod");
}
