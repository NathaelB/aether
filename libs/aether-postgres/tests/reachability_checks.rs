//! `get_checks_since` against a real Postgres: the window bound, the order
//! and the deployment filter are all in its SQL.
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
        ports::DeploymentRepository,
        reachability_history::{ReachabilityCheck, ReachabilityCheckRepository},
    },
    organisation::OrganisationId,
    user::UserId,
    version::Version,
};
use aether_persistence::{SharedTx, in_scratch_tx};
use aether_postgres::{
    dataplane::PostgresDataPlaneRepository,
    deployments::{PostgresDeploymentRepository, PostgresReachabilityChecksRepository},
};
use chrono::{DateTime, Duration, SubsecRound, Utc};
use sqlx::PgPool;
use uuid::Uuid;

mod support;
use support::pool;

fn db_error(error: sqlx::Error) -> CoreError {
    CoreError::DatabaseError {
        message: error.to_string(),
    }
}

async fn data_plane(tx: &SharedTx<'_>) -> DataPlane {
    let dataplane = DataPlane::new(
        DataPlaneAllocation::Shared,
        Region::new("dataplane-actions"),
        Capacity::new(8_000, 16_384, 200).expect("non-zero capacity"),
    );
    PostgresDataPlaneRepository::new(tx)
        .save(&dataplane)
        .await
        .expect("the data plane is saved");
    dataplane
}

async fn a_deployment(tx: &SharedTx<'_>, dataplane_id: DataPlaneId) -> Deployment {
    let organisation_id = OrganisationId(Uuid::new_v4());
    let user_id = UserId(Uuid::new_v4());
    {
        let mut guard = tx.lock().await;
        sqlx::query("INSERT INTO users (id, email, name, sub) VALUES ($1, $2, $3, $4)")
            .bind(user_id.0)
            .bind(format!("{}@reachability-checks.test", user_id.0))
            .bind("reachability-checks")
            .bind(user_id.0.to_string())
            .execute(&mut ***guard)
            .await
            .expect("the user is saved");

        sqlx::query(
            "INSERT INTO organisations \
             (id, name, slug, owner_id, status, plan, max_instances, max_users, \
              max_storage_gb, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, 'active', 'free', 1, 1, 1, now(), now())",
        )
        .bind(organisation_id.0)
        .bind("reachability-checks")
        .bind(organisation_id.0.to_string())
        .bind(user_id.0)
        .execute(&mut ***guard)
        .await
        .expect("the organisation is saved");
    }

    let deployment = deployment(dataplane_id, organisation_id, user_id);
    PostgresDeploymentRepository::new(tx)
        .insert(deployment.clone())
        .await
        .expect("the deployment is saved");
    deployment
}

fn deployment(
    dataplane_id: DataPlaneId,
    organisation_id: OrganisationId,
    created_by: UserId,
) -> Deployment {
    let at = Utc::now();

    Deployment {
        id: DeploymentId(Uuid::new_v4()),
        organisation_id,
        dataplane_id,
        name: DeploymentName("reachability-checks".to_string()),
        kind: DeploymentKind::Ferriskey,
        version: Version::new(26, 0, 1),
        status: DeploymentStatus::InProgress,
        namespace: "reachability-checks-test".to_string(),
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
        last_verified_restore_at: None,
        last_restore_drill_seconds: None,
        log_shipping_enabled: false,
    }
}

async fn scratch<T>(
    pool: &PgPool,
    work: impl AsyncFnOnce(SharedTx<'_>) -> Result<T, CoreError>,
) -> T {
    in_scratch_tx(pool, db_error, work)
        .await
        .expect("the transaction ran")
}

fn check(deployment_id: DeploymentId, at: DateTime<Utc>, reachable: bool) -> ReachabilityCheck {
    ReachabilityCheck {
        deployment_id,
        checked_at: at,
        reachable,
    }
}

#[tokio::test]
async fn checks_since_are_bounded_ordered_and_scoped_to_the_deployment() {
    let Some(pool) = pool().await else {
        return;
    };

    scratch(&pool, async |tx| {
        let dataplane = data_plane(&tx).await;
        let mine = a_deployment(&tx, dataplane.id).await.id;
        let other = a_deployment(&tx, dataplane.id).await.id;
        let repository = PostgresReachabilityChecksRepository::new(&tx);

        let since = Utc::now().trunc_subsecs(6) - Duration::hours(1);
        let at = |minutes: i64| since + Duration::minutes(minutes);

        let inserted = [
            check(mine, at(30), true),
            check(mine, at(-1), false),
            check(mine, at(10), false),
            check(mine, at(0), true),
            check(mine, at(10), true),
            check(other, at(5), false),
        ];
        for check in inserted {
            repository.record_check(check).await?;
        }

        let checks = repository.get_checks_since(mine, since).await?;

        assert_eq!(
            checks,
            vec![
                check(mine, at(0), true),
                check(mine, at(10), false),
                check(mine, at(10), true),
                check(mine, at(30), true),
            ]
        );
        assert!(repository.get_checks_since(mine, at(31)).await?.is_empty());
        Ok(())
    })
    .await;
}
