//! The estate query, against the real tables.
//!
//! The rules are unit tested away from a database. This covers what they
//! cannot: that the join finds the right organisation, that the filters narrow
//! and the cursor pages, and that a deployment nobody should see is not in the
//! answer.
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
        environment::Environment, network::NetworkAccess, ports::DeploymentRepository,
    },
    organisation::OrganisationId,
    platform::{EstateQuery, TenantQuery, ports::EstateRepository},
    upgrades::policy::AutoUpgradePolicy,
    user::UserId,
    version::Version,
};
use aether_persistence::in_scratch_tx;
use aether_postgres::{
    dataplane::PostgresDataPlaneRepository, deployments::PostgresDeploymentRepository,
    platform::PostgresEstateRepository,
};
use chrono::{TimeZone, Utc};
use uuid::Uuid;

mod support;
use support::pool;

const TEST_REGION: &str = "estate-test";

macro_rules! pool_or_skip {
    () => {
        match pool().await {
            Some(pool) => pool,
            None => {
                eprintln!("skipped: DATABASE_URL is not set");
                return;
            }
        }
    };
}

/// The whole point of the endpoint: two organisations, and each deployment
/// carries the one that owns it. Both are called the same thing, because a
/// join that matched on a name rather than an id would pass otherwise.
#[tokio::test]
async fn every_deployment_carries_the_organisation_that_owns_it() {
    let pool = pool_or_skip!();

    let seen: Result<Vec<(String, String)>, CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let dataplane = a_dataplane(&tx).await?;
            let first = an_organisation(&tx, "acme").await?;
            let second = an_organisation(&tx, "globex").await?;

            let deployments = PostgresDeploymentRepository::new(&tx);
            deployments
                .insert(a_deployment(dataplane, first, "auth", 1))
                .await?;
            deployments
                .insert(a_deployment(dataplane, second, "auth", 2))
                .await?;

            let estate = PostgresEstateRepository::new(&tx);
            let mut seen = Vec::new();

            // Asked one organisation at a time, and asserted on what came
            // back rather than on how much did. A test that counted the whole
            // estate would be asserting on whatever else the database holds.
            for owner in [first, second] {
                let page = estate
                    .list_deployments(
                        &EstateQuery::new(None, None)
                            .unwrap()
                            .in_organisation(Some(owner.0)),
                    )
                    .await?;

                seen.extend(
                    page.deployments
                        .into_iter()
                        .map(|estate| (estate.deployment.name.0, estate.organisation.name)),
                );
            }

            Ok(seen)
        },
    )
    .await;

    let seen = seen.expect("the transaction");

    assert_eq!(
        seen,
        vec![
            ("auth".to_string(), "acme".to_string()),
            ("auth".to_string(), "globex".to_string()),
        ],
        "two deployments of the same name, each attributed to its own owner"
    );
}

/// A deployment the data plane has finished tearing down is not part of the
/// estate. One being torn down is: somebody asked for it and is waiting.
#[tokio::test]
async fn what_is_gone_is_not_listed_and_what_is_going_still_is() {
    let pool = pool_or_skip!();

    let statuses: Result<Vec<DeploymentStatus>, CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let dataplane = a_dataplane(&tx).await?;
            let organisation = an_organisation(&tx, "acme").await?;
            let deployments = PostgresDeploymentRepository::new(&tx);

            for (index, status) in [
                DeploymentStatus::Successful,
                DeploymentStatus::Deleting,
                DeploymentStatus::Deleted,
            ]
            .into_iter()
            .enumerate()
            {
                let mut deployment = a_deployment(
                    dataplane,
                    organisation,
                    &format!("d{index}"),
                    index as u128 + 1,
                );
                deployment.status = status;
                deployments.insert(deployment).await?;
            }

            let page = PostgresEstateRepository::new(&tx)
                .list_deployments(
                    &EstateQuery::new(None, None)
                        .unwrap()
                        .in_organisation(Some(organisation.0)),
                )
                .await?;

            Ok(page
                .deployments
                .into_iter()
                .map(|estate| estate.deployment.status)
                .collect())
        },
    )
    .await;

    let mut statuses = statuses.expect("the transaction");
    statuses.sort_by_key(|status| status.to_string());

    assert_eq!(
        statuses,
        [DeploymentStatus::Deleting, DeploymentStatus::Successful]
    );
}

/// The filter that answers "what is on this cluster", which is the question
/// asked before touching one.
#[tokio::test]
async fn a_filter_narrows_to_one_organisation() {
    let pool = pool_or_skip!();

    let names: Result<Vec<String>, CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let dataplane = a_dataplane(&tx).await?;
            let mine = an_organisation(&tx, "acme").await?;
            let theirs = an_organisation(&tx, "globex").await?;

            let deployments = PostgresDeploymentRepository::new(&tx);
            deployments
                .insert(a_deployment(dataplane, mine, "mine", 1))
                .await?;
            deployments
                .insert(a_deployment(dataplane, theirs, "theirs", 2))
                .await?;

            let page = PostgresEstateRepository::new(&tx)
                .list_deployments(
                    &EstateQuery::new(None, None)
                        .unwrap()
                        .in_organisation(Some(mine.0)),
                )
                .await?;

            Ok(page
                .deployments
                .into_iter()
                .map(|estate| estate.deployment.name.0)
                .collect())
        },
    )
    .await;

    assert_eq!(names.expect("the transaction"), vec!["mine".to_string()]);
}

/// A cursor pages without repeating or skipping, and the last page says it is
/// the last rather than leaving the caller to ask once more.
#[tokio::test]
async fn paging_covers_the_estate_exactly_once() {
    let pool = pool_or_skip!();

    let walked: Result<(Vec<String>, bool), CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let dataplane = a_dataplane(&tx).await?;
            let organisation = an_organisation(&tx, "acme").await?;
            let deployments = PostgresDeploymentRepository::new(&tx);

            for index in 0..5u128 {
                let mut deployment =
                    a_deployment(dataplane, organisation, &format!("d{index}"), index + 1);
                // Distinct instants, so "newest first" is a total order and
                // the walk below is reproducible.
                deployment.created_at = Utc
                    .with_ymd_and_hms(2026, 9, 1, 0, 0, index as u32)
                    .unwrap();
                deployments.insert(deployment).await?;
            }

            let estate = PostgresEstateRepository::new(&tx);
            let mut names = Vec::new();
            let mut cursor = None;
            let mut overshot = false;

            loop {
                let page = estate
                    .list_deployments(
                        &EstateQuery::new(Some(2), cursor)
                            .unwrap()
                            .in_organisation(Some(organisation.0)),
                    )
                    .await?;

                names.extend(
                    page.deployments
                        .iter()
                        .map(|estate| estate.deployment.name.0.clone()),
                );

                let Some(next) = page.next_cursor else {
                    break;
                };
                cursor = Some(next);

                // A cursor that never resolves would page for ever. Bounded
                // here rather than by the test runner's timeout, which says
                // nothing about which loop hung.
                if names.len() > 5 {
                    overshot = true;
                    break;
                }
            }

            Ok((names, overshot))
        },
    )
    .await;

    let (names, overshot) = walked.expect("the transaction");

    assert!(!overshot, "the walk did not end where the estate does");
    assert_eq!(
        names,
        vec!["d4", "d3", "d2", "d1", "d0"],
        "newest first, each deployment exactly once"
    );
}

/// The counts are the first thing anybody looks at, and both are easy to get
/// wrong in the same query: joining deployments and members together
/// multiplies the rows, so an organisation with three of one and two of the
/// other reports six of each.
#[tokio::test]
async fn a_tenant_carries_what_it_holds_rather_than_the_product_of_it() {
    let pool = pool_or_skip!();

    let counted: Result<Option<(usize, usize)>, CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let dataplane = a_dataplane(&tx).await?;
            let owner = an_organisation(&tx, "acme").await?;
            let deployments = PostgresDeploymentRepository::new(&tx);

            for index in 0..3u128 {
                deployments
                    .insert(a_deployment(
                        dataplane,
                        owner,
                        &format!("d{index}"),
                        index + 1,
                    ))
                    .await?;
            }

            // Two more members beside the owner's own row.
            for _ in 0..2 {
                a_member(&tx, owner.0).await?;
            }

            let page = PostgresEstateRepository::new(&tx)
                .list_tenants(&TenantQuery::new(None, None).unwrap())
                .await?;

            Ok(page
                .tenants
                .into_iter()
                .find(|tenant| tenant.organisation.id == owner.0)
                .map(|tenant| (tenant.deployments, tenant.members)))
        },
    )
    .await;

    assert_eq!(
        counted.expect("the transaction"),
        Some((3, 2)),
        "three deployments and two members, not their product"
    );
}

/// A deployment the data plane has finished tearing down is not something the
/// organisation holds, and counting it would have an operator believe a tenant
/// still runs what it stopped paying for.
#[tokio::test]
async fn a_tenant_does_not_count_what_is_already_gone() {
    let pool = pool_or_skip!();

    let counted: Result<Option<usize>, CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let dataplane = a_dataplane(&tx).await?;
            let owner = an_organisation(&tx, "acme").await?;
            let deployments = PostgresDeploymentRepository::new(&tx);

            let mut live = a_deployment(dataplane, owner, "live", 1);
            live.status = DeploymentStatus::Successful;
            deployments.insert(live).await?;

            let mut gone = a_deployment(dataplane, owner, "gone", 2);
            gone.status = DeploymentStatus::Deleted;
            deployments.insert(gone).await?;

            let page = PostgresEstateRepository::new(&tx)
                .list_tenants(&TenantQuery::new(None, None).unwrap())
                .await?;

            Ok(page
                .tenants
                .into_iter()
                .find(|tenant| tenant.organisation.id == owner.0)
                .map(|tenant| tenant.deployments))
        },
    )
    .await;

    assert_eq!(counted.expect("the transaction"), Some(1));
}

async fn a_member(
    tx: &aether_persistence::SharedTx<'_>,
    organisation_id: OrganisationId,
) -> Result<(), CoreError> {
    let user_id = UserId(Uuid::new_v4());

    let mut guard = tx.lock().await;
    sqlx::query("INSERT INTO users (id, email, name, sub) VALUES ($1, $2, 'estate', $3)")
        .bind(user_id.0)
        .bind(format!("{}@estate.test", user_id.0))
        .bind(user_id.0.to_string())
        .execute(&mut ***guard)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: e.to_string(),
        })?;

    sqlx::query("INSERT INTO members (id, user_id, organisation_id) VALUES ($1, $2, $3)")
        .bind(Uuid::new_v4())
        .bind(user_id.0)
        .bind(organisation_id.0)
        .execute(&mut ***guard)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: e.to_string(),
        })?;

    Ok(())
}

async fn a_dataplane(tx: &aether_persistence::SharedTx<'_>) -> Result<DataPlaneId, CoreError> {
    let dataplane = DataPlane::new(
        DataPlaneAllocation::Shared,
        Region::new(TEST_REGION),
        Capacity::new(8_000, 16_384, 200).expect("non-zero capacity"),
    );
    PostgresDataPlaneRepository::new(tx)
        .save(&dataplane)
        .await?;

    Ok(dataplane.id)
}

/// An organisation and the user who owns it. The user travels because a
/// deployment's `created_by` is a foreign key, and inventing one per
/// deployment fails at the insert rather than in the assertion.
async fn an_organisation(
    tx: &aether_persistence::SharedTx<'_>,
    name: &str,
) -> Result<(OrganisationId, UserId), CoreError> {
    let organisation_id = OrganisationId(Uuid::new_v4());
    let user_id = UserId(Uuid::new_v4());

    let mut guard = tx.lock().await;
    sqlx::query("INSERT INTO users (id, email, name, sub) VALUES ($1, $2, 'estate', $3)")
        .bind(user_id.0)
        .bind(format!("{}@estate.test", user_id.0))
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
    .bind(name)
    .bind(organisation_id.0.to_string())
    .bind(user_id.0)
    .execute(&mut ***guard)
    .await
    .map_err(|e| CoreError::DatabaseError {
        message: e.to_string(),
    })?;

    Ok((organisation_id, user_id))
}

fn a_deployment(
    dataplane_id: DataPlaneId,
    owner: (OrganisationId, UserId),
    name: &str,
    seed: u128,
) -> Deployment {
    let (organisation_id, created_by) = owner;
    let at = Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap();

    Deployment {
        id: DeploymentId(Uuid::new_v4()),
        organisation_id,
        dataplane_id,
        name: DeploymentName(name.to_string()),
        kind: DeploymentKind::Keycloak,
        version: Version::parse("26.0.0").unwrap(),
        status: DeploymentStatus::Successful,
        environment: Environment::Production,
        namespace: format!("estate-test-{seed}"),
        offer: None,
        restored_from: None,
        resources: DeploymentResources::DEFAULT,
        created_by,
        created_at: at,
        updated_at: at,
        deployed_at: None,
        deleted_at: None,
        auto_upgrade: AutoUpgradePolicy::Manual,
        maintenance_window: None,
        network_access: NetworkAccess::Open,
        last_verified_restore_at: None,
        last_restore_drill_seconds: None,
        log_shipping_enabled: false,
    }
}
