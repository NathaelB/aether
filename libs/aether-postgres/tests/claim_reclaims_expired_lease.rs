//! What a claim picks up, and what it leaves alone.
//!
//! A lease is a promise to publish. Herald claims a batch, publishes it, acks
//! it. If it dies in between -- a restart, a lost broker, an OOM -- nothing
//! acks, and the action stays `Leased` with a deadline nobody is honouring.
//!
//! The rule is one `WHERE` clause and it decides whether that work is ever
//! done again, so it is checked against a real Postgres rather than a mock.
//!
//! Runs only when `DATABASE_URL` is set. See `heartbeat_activates.rs`.

use aether_domain::{
    CoreError,
    action::{
        Action, ActionConstraints, ActionId, ActionMetadata, ActionPayload, ActionSource,
        ActionStatus, ActionTarget, ActionType, ActionVersion, TargetKind, ports::ActionRepository,
    },
    dataplane::{
        entities::DataPlane,
        ports::DataPlaneRepository,
        value_objects::{Capacity, DataPlaneAllocation, DeploymentResources, Region},
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
    action::PostgresActionRepository, dataplane::PostgresDataPlaneRepository,
    deployments::PostgresDeploymentRepository,
};
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use aether_persistence::in_scratch_tx;

mod support;
use support::pool;

/// Every fixture in one run carries this tag. The tests run in parallel
/// against a shared database, so a name reused between them would have one
/// test deleting another's rows mid-flight.
fn tag() -> String {
    format!("reclaim-{}", Uuid::new_v4())
}

/// Saves one action per status, then claims and reports which came back.
///
/// `leased_until` is applied to the `Leased` ones so a test can place the
/// deadline on either side of `now` without waiting for it.
async fn claimed_statuses(
    pool: &PgPool,
    actions: &[(ActionStatus, Option<DateTime<Utc>>)],
) -> Vec<ActionStatus> {
    let actions = actions.to_vec();
    let tag = tag();
    let region = tag.clone();

    let result: Result<Vec<ActionStatus>, CoreError> = in_scratch_tx(
        pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let tag = tag.clone();
            let dataplanes = PostgresDataPlaneRepository::new(&tx);
            let deployments = PostgresDeploymentRepository::new(&tx);
            let repository = PostgresActionRepository::new(&tx);

            let organisation_id = OrganisationId(Uuid::new_v4());
            let user_id = UserId(Uuid::new_v4());
            {
                let mut guard = tx.lock().await;
                sqlx::query("INSERT INTO users (id, email, name, sub) VALUES ($1, $2, $3, $4)")
                    .bind(user_id.0)
                    .bind(format!("{}@reclaim.test", user_id.0))
                    .bind(&tag)
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
                     VALUES ($1, $2, $3, $4, 'active', 'free', 1, 1, 1, now(), now())",
                )
                .bind(organisation_id.0)
                .bind(&tag)
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
                Region::new(&tag),
                Capacity::new(8_000, 16_384, 200).expect("non-zero capacity"),
            );
            dataplanes.save(&dataplane).await?;

            let deployment = deployment(dataplane.id, organisation_id, user_id);
            deployments.insert(deployment.clone()).await?;

            for (index, (status, leased_until)) in actions.iter().enumerate() {
                repository
                    .append(action(
                        deployment.id,
                        dataplane.id,
                        status.clone(),
                        *leased_until,
                        index,
                    ))
                    .await?;
            }

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

            // The claim rewrites `status`, so what identifies a row afterwards
            // is the payload it was created with, not the status it had.
            let mut picked_up = Vec::new();
            for claimed in claimed {
                let index = claimed.payload.data["index"]
                    .as_u64()
                    .expect("every fixture carries its index") as usize;
                picked_up.push(actions[index].0.clone());
            }

            Ok(picked_up)
        },
    )
    .await;

    let picked_up = result.expect("the transaction committed");

    // Order matters: `deployments.created_by` is NOT NULL, so a user cannot go
    // before the deployments pointing at it. Actions cascade with deployments.
    for statement in [
        "DELETE FROM deployments WHERE dataplane_id IN (SELECT id FROM data_planes WHERE region = $1)",
        "DELETE FROM data_planes WHERE region = $1",
        "DELETE FROM organisations WHERE name = $1",
        "DELETE FROM users WHERE name = $1",
    ] {
        sqlx::query(statement)
            .bind(&region)
            .execute(pool)
            .await
            .expect("cleanup");
    }

    picked_up
}

fn deployment(
    dataplane_id: aether_domain::dataplane::value_objects::DataPlaneId,
    organisation_id: OrganisationId,
    created_by: UserId,
) -> Deployment {
    named_deployment(dataplane_id, organisation_id, created_by, "reclaim")
}

/// `name` seeds `hostname_slug`, which is unique per organisation -- a test
/// that saves more than one deployment under the same organisation needs a
/// distinct one for each.
fn named_deployment(
    dataplane_id: aether_domain::dataplane::value_objects::DataPlaneId,
    organisation_id: OrganisationId,
    created_by: UserId,
    name: &str,
) -> Deployment {
    let at = Utc::now();

    Deployment {
        id: DeploymentId(Uuid::new_v4()),
        organisation_id,
        dataplane_id,
        name: DeploymentName(name.to_string()),
        kind: DeploymentKind::Ferriskey,
        version: Version::new(26, 0, 1),
        status: DeploymentStatus::InProgress,
        namespace: "reclaim-test".to_string(),
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

fn action(
    deployment_id: DeploymentId,
    dataplane_id: aether_domain::dataplane::value_objects::DataPlaneId,
    status: ActionStatus,
    leased_until: Option<DateTime<Utc>>,
    index: usize,
) -> Action {
    Action {
        id: ActionId(Uuid::new_v4()),
        deployment_id,
        dataplane_id,
        action_type: ActionType("deployment.create".to_string()),
        target: ActionTarget {
            kind: TargetKind::Deployment,
            id: deployment_id.0,
        },
        payload: ActionPayload {
            data: serde_json::json!({ "index": index }),
        },
        version: ActionVersion(1),
        status,
        metadata: ActionMetadata {
            source: ActionSource::System,
            created_at: Utc::now(),
            constraints: ActionConstraints::default(),
        },
        leased_until,
    }
}

/// The bug this exists to prevent: Herald claims, dies before acking, and the
/// action stays `Leased` for ever. Nothing else in the system reclaims it, so
/// the deployment waits on work no one is doing.
#[tokio::test]
async fn an_expired_lease_is_claimed_again() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let expired = Utc::now() - Duration::seconds(1);

    let picked_up = claimed_statuses(
        &pool,
        &[(ActionStatus::Leased { until: expired }, Some(expired))],
    )
    .await;

    assert_eq!(picked_up.len(), 1, "the expired lease was not reclaimed");
}

/// The other half of the rule. A lease that is still running belongs to a
/// Herald that is still working: claiming it again would publish the same
/// action twice, which is the failure the lease exists to prevent.
#[tokio::test]
async fn a_live_lease_is_left_alone() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let running = Utc::now() + Duration::seconds(600);

    let picked_up = claimed_statuses(
        &pool,
        &[(ActionStatus::Leased { until: running }, Some(running))],
    )
    .await;

    assert!(
        picked_up.is_empty(),
        "a live lease was stolen from its holder: {picked_up:?}"
    );
}

/// A terminal action is done. Reclaiming one would republish work that already
/// happened, and no lease deadline changes that.
#[tokio::test]
async fn a_terminal_action_is_never_claimed() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let at = Utc::now() - Duration::days(1);

    let picked_up = claimed_statuses(
        &pool,
        &[
            (ActionStatus::Published { at }, None),
            (
                ActionStatus::Failed {
                    reason: aether_domain::action::ActionFailureReason::PublishFailed,
                    at,
                },
                None,
            ),
            (ActionStatus::Pending, None),
        ],
    )
    .await;

    assert_eq!(
        picked_up,
        vec![ActionStatus::Pending],
        "only the pending action should have been claimed"
    );
}

/// The point of issue #264: one call claims every deployment a data plane's
/// shard owns, and `max_per_deployment` still caps each deployment on its
/// own rather than the batch as a whole -- so a noisy deployment cannot
/// crowd out a quiet one sharing the same call.
#[tokio::test]
async fn a_claim_covers_every_requested_deployment_and_caps_each_one() {
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let tag = tag();
    let region = tag.clone();

    let result: Result<(usize, usize), CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let dataplanes = PostgresDataPlaneRepository::new(&tx);
            let deployments = PostgresDeploymentRepository::new(&tx);
            let repository = PostgresActionRepository::new(&tx);

            let organisation_id = OrganisationId(Uuid::new_v4());
            let user_id = UserId(Uuid::new_v4());
            {
                let mut guard = tx.lock().await;
                sqlx::query("INSERT INTO users (id, email, name, sub) VALUES ($1, $2, $3, $4)")
                    .bind(user_id.0)
                    .bind(format!("{}@reclaim.test", user_id.0))
                    .bind(&tag)
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
                     VALUES ($1, $2, $3, $4, 'active', 'free', 1, 1, 1, now(), now())",
                )
                .bind(organisation_id.0)
                .bind(&tag)
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
                Region::new(&tag),
                Capacity::new(8_000, 16_384, 200).expect("non-zero capacity"),
            );
            dataplanes.save(&dataplane).await?;

            let noisy = named_deployment(dataplane.id, organisation_id, user_id, "reclaim-noisy");
            let quiet = named_deployment(dataplane.id, organisation_id, user_id, "reclaim-quiet");
            deployments.insert(noisy.clone()).await?;
            deployments.insert(quiet.clone()).await?;

            for index in 0..5 {
                repository
                    .append(action(
                        noisy.id,
                        dataplane.id,
                        ActionStatus::Pending,
                        None,
                        index,
                    ))
                    .await?;
            }
            repository
                .append(action(
                    quiet.id,
                    dataplane.id,
                    ActionStatus::Pending,
                    None,
                    0,
                ))
                .await?;

            let now = Utc::now();
            let claimed = repository
                .claim_pending(
                    dataplane.id,
                    vec![noisy.id, quiet.id],
                    2,
                    now,
                    now + Duration::seconds(60),
                )
                .await?;

            let noisy_claimed = claimed
                .iter()
                .filter(|a| a.deployment_id == noisy.id)
                .count();
            let quiet_claimed = claimed
                .iter()
                .filter(|a| a.deployment_id == quiet.id)
                .count();

            Ok((noisy_claimed, quiet_claimed))
        },
    )
    .await;

    let (noisy_claimed, quiet_claimed) = result.expect("the transaction committed");

    for statement in [
        "DELETE FROM deployments WHERE dataplane_id IN (SELECT id FROM data_planes WHERE region = $1)",
        "DELETE FROM data_planes WHERE region = $1",
        "DELETE FROM organisations WHERE name = $1",
        "DELETE FROM users WHERE name = $1",
    ] {
        sqlx::query(statement)
            .bind(&region)
            .execute(&pool)
            .await
            .expect("cleanup");
    }

    assert_eq!(
        noisy_claimed, 2,
        "max_per_deployment should cap the noisy deployment"
    );
    assert_eq!(
        quiet_claimed, 1,
        "the quiet deployment's single action is still claimed"
    );
}
