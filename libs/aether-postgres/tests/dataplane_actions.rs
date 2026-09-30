//! An action that belongs to a data plane and to no deployment.
//!
//! `actions.deployment_id` used to be `NOT NULL`, and claim, list and ack were
//! all keyed by it. What is checked here is that the same rows can be reached by
//! data plane, and that a deployment's own actions were left alone by it.
//!
//! Runs only when `DATABASE_URL` is set. See `heartbeat_activates.rs`.

use aether_domain::{
    CoreError,
    action::{
        Action, ActionConstraints, ActionCursor, ActionId, ActionMetadata, ActionPayload,
        ActionScope, ActionSource, ActionStatus, ActionTarget, ActionType, ActionVersion,
        TargetKind, ports::ActionRepository,
    },
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
    user::UserId,
    version::Version,
};
use aether_persistence::{SharedTx, in_scratch_tx};
use aether_postgres::{
    action::PostgresActionRepository, dataplane::PostgresDataPlaneRepository,
    deployments::PostgresDeploymentRepository,
};
use chrono::{Duration, Utc};
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
            .bind(format!("{}@dataplane-actions.test", user_id.0))
            .bind("dataplane-actions")
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
        .bind("dataplane-actions")
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
        name: DeploymentName("dataplane-actions".to_string()),
        kind: DeploymentKind::Ferriskey,
        version: Version::new(26, 0, 1),
        status: DeploymentStatus::InProgress,
        namespace: "dataplane-actions-test".to_string(),
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

fn action(deployment_id: Option<DeploymentId>, dataplane_id: DataPlaneId, index: usize) -> Action {
    let (kind, target_id, action_type) = match deployment_id {
        Some(id) => (
            TargetKind::Deployment,
            id.0,
            ActionType("deployment.create".to_string()),
        ),
        None => (
            TargetKind::DataPlane,
            dataplane_id.0,
            ActionType::dataplane_upgrade(),
        ),
    };

    Action {
        id: ActionId(Uuid::new_v4()),
        deployment_id,
        dataplane_id,
        action_type,
        target: ActionTarget {
            kind,
            id: target_id,
        },
        payload: ActionPayload {
            data: serde_json::json!({ "index": index }),
        },
        version: ActionVersion(1),
        status: ActionStatus::Pending,
        metadata: ActionMetadata {
            source: ActionSource::System,
            created_at: Utc::now() + Duration::milliseconds(index as i64),
            constraints: ActionConstraints::default(),
        },
        leased_until: None,
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

#[tokio::test]
async fn a_data_plane_action_is_created_claimed_and_acked() {
    let Some(pool) = pool().await else {
        return;
    };

    scratch(&pool, async |tx| {
        let dataplane = data_plane(&tx).await;
        let repository = PostgresActionRepository::new(&tx);
        let upgrade = action(None, dataplane.id, 0);
        repository.append(upgrade.clone()).await?;

        let now = Utc::now();
        let claimed = repository
            .claim_dataplane_pending(dataplane.id, 10, now, now + Duration::seconds(60))
            .await?;

        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].id, upgrade.id);
        assert_eq!(claimed[0].deployment_id, None);
        assert_eq!(claimed[0].target.kind, TargetKind::DataPlane);
        assert!(matches!(claimed[0].status, ActionStatus::Leased { .. }));

        let again = repository
            .claim_dataplane_pending(dataplane.id, 10, now, now + Duration::seconds(60))
            .await?;
        assert!(again.is_empty(), "a live lease was claimed twice");

        let acked = repository
            .ack_published(ActionScope::DataPlane(dataplane.id), upgrade.id, now)
            .await?;
        assert!(acked);

        let acked_twice = repository
            .ack_published(ActionScope::DataPlane(dataplane.id), upgrade.id, now)
            .await?;
        assert!(!acked_twice, "a published action was acked again");

        let listed = repository
            .list(ActionScope::DataPlane(dataplane.id), None, 10)
            .await?;
        assert_eq!(listed.actions.len(), 1);
        assert!(matches!(
            listed.actions[0].status,
            ActionStatus::Published { .. }
        ));

        Ok(())
    })
    .await;
}

#[tokio::test]
async fn a_failed_data_plane_action_is_recorded_as_failed() {
    let Some(pool) = pool().await else {
        return;
    };

    scratch(&pool, async |tx| {
        let dataplane = data_plane(&tx).await;
        let repository = PostgresActionRepository::new(&tx);
        let upgrade = action(None, dataplane.id, 0);
        repository.append(upgrade.clone()).await?;

        let now = Utc::now();
        repository
            .claim_dataplane_pending(dataplane.id, 10, now, now + Duration::seconds(60))
            .await?;

        let acked = repository
            .ack_failed(
                ActionScope::DataPlane(dataplane.id),
                upgrade.id,
                aether_domain::action::ActionFailureReason::PublishFailed,
                now,
            )
            .await?;
        assert!(acked);

        let listed = repository
            .list(ActionScope::DataPlane(dataplane.id), None, 10)
            .await?;
        assert!(matches!(
            listed.actions[0].status,
            ActionStatus::Failed { .. }
        ));

        Ok(())
    })
    .await;
}

#[tokio::test]
async fn a_data_plane_action_belongs_to_its_data_plane_alone() {
    let Some(pool) = pool().await else {
        return;
    };

    scratch(&pool, async |tx| {
        let mine = data_plane(&tx).await;
        let other = data_plane(&tx).await;
        let repository = PostgresActionRepository::new(&tx);
        let upgrade = action(None, mine.id, 0);
        repository.append(upgrade.clone()).await?;

        let now = Utc::now();
        let stolen = repository
            .claim_dataplane_pending(other.id, 10, now, now + Duration::seconds(60))
            .await?;
        assert!(stolen.is_empty());

        repository
            .claim_dataplane_pending(mine.id, 10, now, now + Duration::seconds(60))
            .await?;
        let acked = repository
            .ack_published(ActionScope::DataPlane(other.id), upgrade.id, now)
            .await?;
        assert!(!acked, "another data plane acked it");

        let listed = repository
            .list(ActionScope::DataPlane(other.id), None, 10)
            .await?;
        assert!(listed.actions.is_empty());

        Ok(())
    })
    .await;
}

#[tokio::test]
async fn deployment_actions_are_untouched_by_data_plane_ones() {
    let Some(pool) = pool().await else {
        return;
    };

    scratch(&pool, async |tx| {
        let dataplane = data_plane(&tx).await;
        let deployment = a_deployment(&tx, dataplane.id).await;
        let repository = PostgresActionRepository::new(&tx);
        let create = action(Some(deployment.id), dataplane.id, 0);
        let upgrade = action(None, dataplane.id, 1);
        repository.append(create.clone()).await?;
        repository.append(upgrade.clone()).await?;

        let now = Utc::now();
        let claimed = repository
            .claim_pending(
                dataplane.id,
                vec![deployment.id],
                10,
                now,
                now + Duration::seconds(60),
            )
            .await?;
        assert_eq!(
            claimed.len(),
            1,
            "claim by deployment took the data plane action"
        );
        assert_eq!(claimed[0].id, create.id);
        assert_eq!(claimed[0].deployment_id, Some(deployment.id));

        let by_deployment = repository
            .list(ActionScope::Deployment(deployment.id), None, 10)
            .await?;
        assert_eq!(by_deployment.actions.len(), 1);
        assert_eq!(by_deployment.actions[0].id, create.id);

        let wrong_ack = repository
            .ack_published(ActionScope::DataPlane(dataplane.id), create.id, now)
            .await?;
        assert!(!wrong_ack, "a data plane ack reached a deployment action");

        let acked = repository
            .ack_published(ActionScope::Deployment(deployment.id), create.id, now)
            .await?;
        assert!(acked);

        let by_data_plane = repository
            .list(ActionScope::DataPlane(dataplane.id), None, 10)
            .await?;
        assert_eq!(by_data_plane.actions.len(), 1);
        assert_eq!(by_data_plane.actions[0].id, upgrade.id);

        Ok(())
    })
    .await;
}

#[tokio::test]
async fn listing_by_data_plane_pages_with_a_cursor() {
    let Some(pool) = pool().await else {
        return;
    };

    scratch(&pool, async |tx| {
        let dataplane = data_plane(&tx).await;
        let repository = PostgresActionRepository::new(&tx);
        for index in 0..3 {
            repository.append(action(None, dataplane.id, index)).await?;
        }

        let first = repository
            .list(ActionScope::DataPlane(dataplane.id), None, 2)
            .await?;
        assert_eq!(first.actions.len(), 2);

        let cursor: ActionCursor = first.next_cursor.expect("a cursor");
        let rest = repository
            .list(ActionScope::DataPlane(dataplane.id), Some(cursor), 2)
            .await?;
        assert_eq!(rest.actions.len(), 1);
        assert_eq!(rest.actions[0].payload.data["index"], 2);

        Ok(())
    })
    .await;
}

#[tokio::test]
async fn an_action_with_neither_a_deployment_nor_a_data_plane_target_is_refused() {
    let Some(pool) = pool().await else {
        return;
    };

    scratch(&pool, async |tx| {
        let dataplane = data_plane(&tx).await;
        let repository = PostgresActionRepository::new(&tx);
        let mut orphan = action(None, dataplane.id, 0);
        orphan.target.kind = TargetKind::Deployment;

        let result = repository.append(orphan).await;
        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));

        Ok(())
    })
    .await;
}
