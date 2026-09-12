//! What a membership grants, against the real join.
//!
//! The unit tests cover the rule with stubs. This covers what they cannot:
//! that `members`, `member_roles` and `roles` actually meet, and that a member
//! holding no role comes back as a member rather than as nobody.
//!
//! Runs only when `DATABASE_URL` is set. See `heartbeat_activates.rs`.

use aether_domain::{
    CoreError,
    organisation::{OrganisationId, ports::OrganisationRepository},
    role::{Role, RoleId, ports::RoleRepository},
    user::UserId,
};
use aether_persistence::with_tx;
use aether_postgres::{organisation::PostgresOrganisationRepository, role::PostgresRoleRepository};
use chrono::Utc;
use uuid::Uuid;

mod support;
use support::pool;

/// One tag per test rather than one for the suite.
///
/// The tests run in parallel and each cleans up by tag; sharing one meant
/// every test deleting the others' rows while they were still using them.
/// Passing alone and failing together is exactly what that looks like.
fn tag(test: &str) -> String {
    format!("member-roles-{test}")
}

struct Fixture {
    organisation: OrganisationId,
    owner: UserId,
    member: UserId,
}

/// A member granted two roles reads back holding both, and nothing else.
#[tokio::test]
async fn a_membership_carries_the_roles_it_was_granted() {
    let tag = tag("a_membership_carries_the_roles_it_was_granted");
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let held: Result<Vec<String>, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let organisations = PostgresOrganisationRepository::new(&tx);
            let roles = PostgresRoleRepository::new(&tx);
            let fixture = seed(&tx, &tag).await?;

            let viewer = role(fixture.organisation, "viewer", 4);
            let operator = role(fixture.organisation, "operator", 16);
            roles.insert(viewer.clone()).await?;
            roles.insert(operator.clone()).await?;
            grant(&tx, &fixture, &[viewer.id, operator.id]).await?;

            let member = organisations
                .find_member(&fixture.organisation, &fixture.member)
                .await?
                .expect("a member");

            let mut names: Vec<String> = member.roles.iter().map(|r| r.name.clone()).collect();
            names.sort();
            Ok(names)
        },
    )
    .await;

    clean_up(&pool, &tag).await;

    assert_eq!(held.expect("the transaction"), vec!["operator", "viewer"]);
}

/// The permissions the join adds up to. A test that fails if the query drops
/// a row, which a plain inner join on two roles would not show.
#[tokio::test]
async fn the_permissions_of_a_membership_are_the_union_of_its_roles() {
    let tag = tag("the_permissions_of_a_membership_are_the_union_of_its_roles");
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let permissions: Result<u64, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let organisations = PostgresOrganisationRepository::new(&tx);
            let roles = PostgresRoleRepository::new(&tx);
            let fixture = seed(&tx, &tag).await?;

            let one = role(fixture.organisation, "one", 0b0001);
            let two = role(fixture.organisation, "two", 0b0100);
            roles.insert(one.clone()).await?;
            roles.insert(two.clone()).await?;
            grant(&tx, &fixture, &[one.id, two.id]).await?;

            let member = organisations
                .find_member(&fixture.organisation, &fixture.member)
                .await?
                .expect("a member");

            Ok(member.permissions())
        },
    )
    .await;

    clean_up(&pool, &tag).await;

    assert_eq!(permissions.expect("the transaction"), 0b0101);
}

/// The distinction the left join exists for. An inner join would answer
/// "nobody" here, and "in the organisation, granted nothing" would become
/// indistinguishable from "not in it".
#[tokio::test]
async fn a_member_granted_nothing_is_still_a_member() {
    let tag = tag("a_member_granted_nothing_is_still_a_member");
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let found: Result<bool, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let organisations = PostgresOrganisationRepository::new(&tx);
            let fixture = seed(&tx, &tag).await?;

            let member = organisations
                .find_member(&fixture.organisation, &fixture.member)
                .await?;

            Ok(member.is_some_and(|member| member.roles.is_empty()))
        },
    )
    .await;

    clean_up(&pool, &tag).await;

    assert!(
        found.expect("the transaction"),
        "a member with no roles was read as nobody"
    );
}

#[tokio::test]
async fn somebody_who_never_joined_is_not_a_member() {
    let tag = tag("somebody_who_never_joined_is_not_a_member");
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let found: Result<bool, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let organisations = PostgresOrganisationRepository::new(&tx);
            let fixture = seed(&tx, &tag).await?;

            // The owner holds no member row here: seed only puts the second
            // user in. Ownership is a different rule and does not need one.
            let member = organisations
                .find_member(&fixture.organisation, &fixture.owner)
                .await?;

            Ok(member.is_none())
        },
    )
    .await;

    clean_up(&pool, &tag).await;

    assert!(found.expect("the transaction"));
}

/// Deleting a role takes the grant with it rather than leaving a membership
/// pointing at nothing.
#[tokio::test]
async fn deleting_a_role_leaves_the_member_holding_fewer() {
    let tag = tag("deleting_a_role_leaves_the_member_holding_fewer");
    let Some(pool) = pool().await else {
        eprintln!("skipped: DATABASE_URL is not set");
        return;
    };

    let left: Result<usize, CoreError> = with_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let organisations = PostgresOrganisationRepository::new(&tx);
            let roles = PostgresRoleRepository::new(&tx);
            let fixture = seed(&tx, &tag).await?;

            let kept = role(fixture.organisation, "kept", 4);
            let dropped = role(fixture.organisation, "dropped", 16);
            roles.insert(kept.clone()).await?;
            roles.insert(dropped.clone()).await?;
            grant(&tx, &fixture, &[kept.id, dropped.id]).await?;

            roles.delete(dropped.id).await?;

            let member = organisations
                .find_member(&fixture.organisation, &fixture.member)
                .await?
                .expect("still a member");

            Ok(member.roles.len())
        },
    )
    .await;

    clean_up(&pool, &tag).await;

    assert_eq!(left.expect("the transaction"), 1);
}

fn role(organisation: OrganisationId, name: &str, permissions: u64) -> Role {
    Role {
        id: RoleId(Uuid::new_v4()),
        name: name.to_string(),
        permissions,
        organisation_id: Some(organisation),
        color: None,
        created_at: Utc::now(),
    }
}

async fn grant(
    tx: &aether_persistence::SharedTx<'_>,
    fixture: &Fixture,
    roles: &[RoleId],
) -> Result<(), CoreError> {
    let mut guard = tx.lock().await;

    for role_id in roles {
        sqlx::query(
            "INSERT INTO member_roles (member_id, role_id) \
             SELECT id, $3 FROM members WHERE organisation_id = $1 AND user_id = $2",
        )
        .bind(fixture.organisation.0)
        .bind(fixture.member.0)
        .bind(role_id.0)
        .execute(&mut ***guard)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: e.to_string(),
        })?;
    }

    Ok(())
}

async fn seed(tx: &aether_persistence::SharedTx<'_>, tag: &str) -> Result<Fixture, CoreError> {
    let organisations = PostgresOrganisationRepository::new(tx);
    let owner = UserId(Uuid::new_v4());
    let member = UserId(Uuid::new_v4());
    let organisation = OrganisationId(Uuid::new_v4());

    {
        let mut guard = tx.lock().await;
        for (id, label) in [(owner.0, "owner"), (member.0, "member")] {
            sqlx::query("INSERT INTO users (id, email, name, sub) VALUES ($1, $2, $3, $4)")
                .bind(id)
                .bind(format!("{id}@{tag}"))
                .bind(tag)
                .bind(format!("{tag}-{label}-{id}"))
                .execute(&mut ***guard)
                .await
                .map_err(|e| CoreError::DatabaseError {
                    message: e.to_string(),
                })?;
        }

        sqlx::query(
            "INSERT INTO organisations \
             (id, name, slug, owner_id, status, plan, max_instances, max_users, \
              max_storage_gb, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, 'active', 'free', 5, 5, 5, now(), now())",
        )
        .bind(organisation.0)
        .bind(tag)
        .bind(organisation.0.to_string())
        .bind(owner.0)
        .execute(&mut ***guard)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: e.to_string(),
        })?;
    }

    organisations.insert_member(&organisation, &member).await?;

    Ok(Fixture {
        organisation,
        owner,
        member,
    })
}

/// `with_tx` commits, so a suite that does not clean up leaves its rows behind.
async fn clean_up(pool: &sqlx::PgPool, tag: &str) {
    for statement in [
        "DELETE FROM roles WHERE organisation_id IN (SELECT id FROM organisations WHERE name = $1)",
        "DELETE FROM members WHERE organisation_id IN (SELECT id FROM organisations WHERE name = $1)",
        "DELETE FROM organisations WHERE name = $1",
        "DELETE FROM users WHERE name = $1",
    ] {
        sqlx::query(statement)
            .bind(tag)
            .execute(pool)
            .await
            .expect("cleanup");
    }
}
