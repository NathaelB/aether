//! The optional deployment-count bound (#270), checked against a real
//! Postgres rather than a mock.
//!
//! The exclusion and the "why" both live in hand-written SQL -- a `HAVING`
//! clause and its inverse -- so no domain unit test reaches them. This is the
//! shape of logic that has to meet the real engine.
//!
//! Runs only when `DATABASE_URL` is set. See `heartbeat_activates.rs`.

use aether_domain::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        ports::DataPlaneRepository,
        value_objects::{
            Capacity, DataPlaneAllocation, DataPlaneMode, DataPlaneStatus, DeploymentResources,
            PlacementPolicy, PlacementRequest, Region,
        },
    },
    deployments::{
        Deployment, DeploymentId, DeploymentKind, DeploymentName, DeploymentStatus,
        ports::DeploymentRepository,
    },
    organisation::OrganisationId,
    user::UserId,
    version::Version,
};
use aether_postgres::{
    dataplane::PostgresDataPlaneRepository, deployments::PostgresDeploymentRepository,
};
use chrono::{Duration, Utc};
use uuid::Uuid;

use aether_persistence::in_scratch_tx;

mod support;
use support::pool;

const TEST_REGION: &str = "deployment-count-bound-test";

async fn seed_user_and_organisation(
    tx: &aether_persistence::SharedTx<'_>,
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
         VALUES ($1, 'bound', $2, $3, 'active', 'free', 1, 1, 1, now(), now())",
    )
    .bind(organisation_id.0)
    .bind(organisation_id.0.to_string())
    .bind(user_id.0)
    .execute(&mut ***guard)
    .await
    .map_err(|e| CoreError::DatabaseError {
        message: e.to_string(),
    })?;

    Ok((user_id, organisation_id))
}

fn placed_deployment(
    dataplane_id: aether_domain::dataplane::value_objects::DataPlaneId,
    organisation_id: OrganisationId,
    created_by: UserId,
) -> Deployment {
    let at = Utc::now();

    Deployment {
        id: DeploymentId(Uuid::new_v4()),
        organisation_id,
        dataplane_id,
        name: DeploymentName("bound".to_string()),
        kind: DeploymentKind::Ferriskey,
        version: Version::new(26, 0, 1),
        status: DeploymentStatus::Successful,
        namespace: "deployment-count-bound-test".to_string(),
        environment: aether_domain::deployments::environment::Environment::Development,
        offer: None,
        restored_from: None,
        resources: DeploymentResources::DEFAULT,
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

/// The point of the issue: a plane at its count is excluded from placement
/// even though every resource dimension still has room to spare, and the
/// repository can say the count is why.
#[tokio::test]
async fn a_plane_at_its_count_is_excluded_with_resources_to_spare() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let result: Result<(bool, bool), CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let dataplanes = PostgresDataPlaneRepository::new(&tx);
            let deployments = PostgresDeploymentRepository::new(&tx);

            // Plenty by every resource measure -- the count is the only
            // thing standing in the way.
            let capacity = Capacity::new(64_000, 131_072, 2_000)
                .expect("non-zero capacity")
                .with_max_deployments(1)
                .expect("non-zero bound");

            let mut dataplane = DataPlane::new(
                DataPlaneAllocation::Shared,
                Region::new(TEST_REGION),
                capacity,
            );
            dataplane.status = DataPlaneStatus::Active;
            dataplanes.save(&dataplane).await?;
            // `save` never writes `last_seen_at` -- that column is a
            // heartbeat's business, not a registration's. `find_available`
            // requires it within the window, same as `heartbeat_activates.rs`.
            dataplanes
                .touch_last_seen(&dataplane.id, Utc::now(), None, None)
                .await?;

            let (user_id, organisation_id) = seed_user_and_organisation(&tx).await?;
            deployments
                .insert(placed_deployment(dataplane.id, organisation_id, user_id))
                .await?;

            let found = dataplanes
                .find_available(PlacementRequest {
                    region: Some(Region::new(TEST_REGION)),
                    organisation_id,
                    mode: DataPlaneMode::Shared,
                    resources: DeploymentResources::DEFAULT,
                    policy: PlacementPolicy::default(),
                    seen_since: Utc::now() - Duration::seconds(90),
                })
                .await?;

            let blocked = dataplanes
                .region_blocked_by_deployment_count(
                    &Region::new(TEST_REGION),
                    DataPlaneMode::Shared,
                    DeploymentResources::DEFAULT,
                )
                .await?;

            Ok((found.is_some(), blocked))
        },
    )
    .await;

    let (found_a_plane, blocked_by_count) = result.expect("the transaction committed");

    assert!(
        !found_a_plane,
        "a plane at its count must not be handed back as available"
    );
    assert!(
        blocked_by_count,
        "the repository must be able to say the count, not resources, is why"
    );
}

/// The other half of the acceptance: a plane with no count bound behaves
/// exactly as it did before this issue, and is never reported as blocked by
/// a count that was never set.
#[tokio::test]
async fn a_plane_with_no_count_bound_is_never_reported_as_blocked_by_count() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let result: Result<(bool, bool), CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let dataplanes = PostgresDataPlaneRepository::new(&tx);
            let deployments = PostgresDeploymentRepository::new(&tx);

            let capacity = Capacity::new(64_000, 131_072, 2_000).expect("non-zero capacity");

            let mut dataplane = DataPlane::new(
                DataPlaneAllocation::Shared,
                Region::new(TEST_REGION),
                capacity,
            );
            dataplane.status = DataPlaneStatus::Active;
            dataplanes.save(&dataplane).await?;
            // `save` never writes `last_seen_at` -- that column is a
            // heartbeat's business, not a registration's. `find_available`
            // requires it within the window, same as `heartbeat_activates.rs`.
            dataplanes
                .touch_last_seen(&dataplane.id, Utc::now(), None, None)
                .await?;

            let (user_id, organisation_id) = seed_user_and_organisation(&tx).await?;
            deployments
                .insert(placed_deployment(dataplane.id, organisation_id, user_id))
                .await?;

            let found = dataplanes
                .find_available(PlacementRequest {
                    region: Some(Region::new(TEST_REGION)),
                    organisation_id,
                    mode: DataPlaneMode::Shared,
                    resources: DeploymentResources::DEFAULT,
                    policy: PlacementPolicy::default(),
                    seen_since: Utc::now() - Duration::seconds(90),
                })
                .await?;

            let blocked = dataplanes
                .region_blocked_by_deployment_count(
                    &Region::new(TEST_REGION),
                    DataPlaneMode::Shared,
                    DeploymentResources::DEFAULT,
                )
                .await?;

            Ok((found.is_some(), blocked))
        },
    )
    .await;

    let (found_a_plane, blocked_by_count) = result.expect("the transaction committed");

    assert!(found_a_plane, "no bound must not stop ordinary placement");
    assert!(
        !blocked_by_count,
        "a plane with no bound was never blocked by one"
    );
}

/// Round-tripped through a real row: `None` stays `None`, and a bound
/// survives being written and read back.
#[tokio::test]
async fn the_count_bound_round_trips_through_a_saved_row() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let result: Result<(Option<u32>, Option<u32>), CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let dataplanes = PostgresDataPlaneRepository::new(&tx);

            let unbounded = DataPlane::new(
                DataPlaneAllocation::Shared,
                Region::new(TEST_REGION),
                Capacity::new(4_000, 8_192, 100).expect("non-zero capacity"),
            );
            dataplanes.save(&unbounded).await?;

            let bounded = DataPlane::new(
                DataPlaneAllocation::Shared,
                Region::new(TEST_REGION),
                Capacity::new(4_000, 8_192, 100)
                    .expect("non-zero capacity")
                    .with_max_deployments(3)
                    .expect("non-zero bound"),
            );
            dataplanes.save(&bounded).await?;

            let reloaded_unbounded = dataplanes
                .find_by_id(&unbounded.id)
                .await?
                .expect("just saved");
            let reloaded_bounded = dataplanes
                .find_by_id(&bounded.id)
                .await?
                .expect("just saved");

            Ok((
                reloaded_unbounded.capacity.max_deployments(),
                reloaded_bounded.capacity.max_deployments(),
            ))
        },
    )
    .await;

    let (unbounded, bounded) = result.expect("the transaction committed");

    assert_eq!(unbounded, None, "absent means exactly today's behaviour");
    assert_eq!(bounded, Some(3));
}
