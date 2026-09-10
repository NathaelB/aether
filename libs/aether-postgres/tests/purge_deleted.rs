//! What the retention purge removes, and what it must not.
//!
//! The rule is SQL, and it deletes rows permanently -- `actions` cascade with
//! them -- so it is checked against a real Postgres rather than a mock.
//!
//! Runs only when `DATABASE_URL` is set. See `heartbeat_activates.rs`.

use aether_domain::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        ports::DataPlaneRepository,
        value_objects::{Capacity, DataPlaneAllocation, Region},
    },
    deployments::{
        Deployment, DeploymentId, DeploymentKind, DeploymentName, DeploymentStatus,
        DeploymentVersion, ports::DeploymentRepository,
    },
    organisation::OrganisationId,
    user::UserId,
};
use aether_persistence::with_tx;
use aether_postgres::{
    dataplane::PostgresDataPlaneRepository, deployments::PostgresDeploymentRepository,
};
use chrono::{Duration, Utc};
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

const TEST_REGION: &str = "purge-deleted-test";

async fn pool() -> Option<PgPool> {
    let url = std::env::var("DATABASE_URL")
        .ok()
        .filter(|u| !u.is_empty())?;

    Some(
        PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .expect("DATABASE_URL is set but the database is unreachable"),
    )
}

/// Saves one deployment per status, ages them all past the retention window,
/// purges, and reports which survived.
async fn survivors_after_purge(
    pool: &PgPool,
    statuses: &[DeploymentStatus],
) -> Vec<DeploymentStatus> {
    let statuses = statuses.to_vec();

    let result: Result<Vec<DeploymentStatus>, CoreError> = with_tx(
        pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let dataplanes = PostgresDataPlaneRepository::new(&tx);
            let deployments = PostgresDeploymentRepository::new(&tx);

            // Deployments reference an organisation and a user. Neither has a
            // repository this test needs, so both are inserted directly.
            let organisation_id = OrganisationId(Uuid::new_v4());
            let user_id = UserId(Uuid::new_v4());
            {
                let mut guard = tx.lock().await;
                sqlx::query(
                    "INSERT INTO users (id, email, name, sub) VALUES ($1, $2, 'purge', $3)",
                )
                .bind(user_id.0)
                .bind(format!("{}@purge.test", user_id.0))
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
                     VALUES ($1, 'purge', $2, $3, 'active', 'free', 1, 1, 1, now(), now())",
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

            let dataplane = DataPlane::new(
                DataPlaneAllocation::Shared,
                Region::new(TEST_REGION),
                Capacity::new(8_000, 16_384, 200).expect("non-zero capacity"),
            );
            dataplanes.save(&dataplane).await?;

            let mut saved = Vec::new();
            for status in &statuses {
                let deployment = deployment(dataplane.id, organisation_id, user_id, status.clone());
                deployments.insert(deployment.clone()).await?;
                saved.push(deployment);
            }

            // Age every row past the retention window in one statement: the
            // repository has no way to write `updated_at` directly, and the
            // alternative is waiting a month.
            {
                let mut guard = tx.lock().await;
                sqlx::query("UPDATE deployments SET updated_at = $1 WHERE dataplane_id = $2")
                    .bind(Utc::now() - Duration::days(90))
                    .bind(dataplane.id.0)
                    .execute(&mut ***guard)
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
            }

            deployments
                .purge_deleted(Utc::now() - Duration::days(30))
                .await?;

            let mut survivors = Vec::new();
            for deployment in saved {
                if let Some(found) = deployments.get_by_id(deployment.id).await? {
                    survivors.push(found.status);
                }
            }

            Ok(survivors)
        },
    )
    .await;

    let survivors = result.expect("the transaction committed");

    sqlx::query("DELETE FROM organisations WHERE name = 'purge'")
        .execute(pool)
        .await
        .expect("cleanup");
    sqlx::query("DELETE FROM users WHERE name = 'purge'")
        .execute(pool)
        .await
        .expect("cleanup");
    sqlx::query("DELETE FROM data_planes WHERE region = $1")
        .bind(TEST_REGION)
        .execute(pool)
        .await
        .expect("cleanup");

    survivors
}

fn deployment(
    dataplane_id: aether_domain::dataplane::value_objects::DataPlaneId,
    organisation_id: OrganisationId,
    created_by: UserId,
    status: DeploymentStatus,
) -> Deployment {
    let at = Utc::now();

    Deployment {
        id: DeploymentId(Uuid::new_v4()),
        organisation_id,
        dataplane_id,
        name: DeploymentName("purge".to_string()),
        kind: DeploymentKind::Ferriskey,
        version: DeploymentVersion("latest".to_string()),
        status,
        namespace: "purge-test".to_string(),
        resources: aether_domain::dataplane::value_objects::DeploymentResources::DEFAULT,
        created_by,
        created_at: at,
        updated_at: at,
        deployed_at: None,
        deleted_at: None,
    }
}

/// A deployment still waiting for its tear-down to be confirmed is one that
/// needs attention. Purging it would erase the evidence that something is
/// stuck, which is the opposite of what a retention policy is for.
#[tokio::test]
async fn only_a_confirmed_deletion_is_purged() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let survivors = survivors_after_purge(
        &pool,
        &[
            DeploymentStatus::Deleted,
            DeploymentStatus::Deleting,
            DeploymentStatus::Successful,
            DeploymentStatus::Failed,
            DeploymentStatus::Pending,
        ],
    )
    .await;

    assert!(
        !survivors.contains(&DeploymentStatus::Deleted),
        "a confirmed deletion past its retention is removed"
    );
    assert_eq!(
        survivors.len(),
        4,
        "everything else survives, whatever its age: {survivors:?}"
    );
}
