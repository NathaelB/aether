//! What the organisation's deployment list shows, and what it hides.
//!
//! Runs only when `DATABASE_URL` is set. See `heartbeat_activates.rs`.

use aether_domain::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        ports::DataPlaneRepository,
        value_objects::{Capacity, DataPlaneAllocation, DataPlaneId, Region},
    },
    deployments::{
        Deployment, DeploymentId, DeploymentKind, DeploymentName, DeploymentStatus,
        ports::DeploymentRepository,
    },
    organisation::OrganisationId,
    user::UserId,
    version::Version,
};
use aether_persistence::with_tx;
use aether_postgres::{
    dataplane::PostgresDataPlaneRepository, deployments::PostgresDeploymentRepository,
};
use chrono::Utc;
use uuid::Uuid;

mod support;
use support::pool;

const TEST_REGION: &str = "list-excludes-deleted-test";

/// A tear-down in flight is something the customer asked for and is waiting on.
/// It stays visible until the data plane confirms it is gone -- filtering on
/// `deleted_at`, which is stamped when the deletion is *requested*, hid it from
/// the moment the button was pressed.
#[tokio::test]
async fn a_deployment_being_torn_down_stays_visible_until_it_is_gone() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let listed: Result<Vec<DeploymentStatus>, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let dataplanes = PostgresDataPlaneRepository::new(&tx);
            let deployments = PostgresDeploymentRepository::new(&tx);

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
                sqlx::query("INSERT INTO users (id, email, name, sub) VALUES ($1, $2, 'list', $3)")
                    .bind(user_id.0)
                    .bind(format!("{}@list.test", user_id.0))
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
                     VALUES ($1, 'list', $2, $3, 'active', 'free', 1, 1, 1, now(), now())",
                )
                .bind(organisation_id.0)
                .bind(organisation_id.0.to_string())
                .bind(user_id.0)
                .execute(&mut ***guard)
                .await
                .map_err(|e| CoreError::DatabaseError {
                    message: e.to_string(),
                })?;
            }

            for status in [
                DeploymentStatus::Successful,
                DeploymentStatus::Deleting,
                DeploymentStatus::Deleted,
            ] {
                let mut deployment =
                    deployment(dataplane.id, organisation_id, user_id, status.clone());

                // Both a tear-down in flight and a finished one carry
                // `deleted_at`: it marks the request, not the outcome. That is
                // exactly why the list cannot filter on it.
                if status != DeploymentStatus::Successful {
                    deployment.deleted_at = Some(Utc::now());
                }

                deployments.insert(deployment).await?;
            }

            let listed = deployments.list_by_organisation(organisation_id).await?;

            Ok(listed.into_iter().map(|d| d.status).collect())
        },
    )
    .await;

    let mut listed = listed.expect("the transaction committed");
    listed.sort_by_key(|status| status.to_string());

    sqlx::query("DELETE FROM organisations WHERE name = 'list'")
        .execute(&pool)
        .await
        .expect("cleanup");
    sqlx::query("DELETE FROM users WHERE name = 'list'")
        .execute(&pool)
        .await
        .expect("cleanup");
    sqlx::query("DELETE FROM data_planes WHERE region = $1")
        .bind(TEST_REGION)
        .execute(&pool)
        .await
        .expect("cleanup");

    assert_eq!(
        listed,
        [DeploymentStatus::Deleting, DeploymentStatus::Successful],
        "deleting is shown, deleted is not"
    );
}

fn deployment(
    dataplane_id: DataPlaneId,
    organisation_id: OrganisationId,
    created_by: UserId,
    status: DeploymentStatus,
) -> Deployment {
    let at = Utc::now();

    Deployment {
        id: DeploymentId(Uuid::new_v4()),
        organisation_id,
        dataplane_id,
        name: DeploymentName("listed".to_string()),
        kind: DeploymentKind::Ferriskey,
        version: Version::new(26, 0, 1),
        status,
        namespace: "list-test".to_string(),
        resources: aether_domain::dataplane::value_objects::DeploymentResources::DEFAULT,
        created_by,
        created_at: at,
        updated_at: at,
        deployed_at: None,
        deleted_at: None,
    }
}
