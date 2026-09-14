//! Who operates the installation, against the real table.
//!
//! The rules are unit tested away from a database. This covers what they
//! cannot: that a grant replaces rather than collides, that the counting the
//! last-administrator rule depends on is right, and that a right the code does
//! not know about is refused rather than silently dropped.
//!
//! Runs only when `DATABASE_URL` is set. See `heartbeat_activates.rs`.

use aether_domain::{
    CoreError,
    platform::{PlatformOperator, PlatformRight, PlatformRights, ports::OperatorRepository},
};
use aether_persistence::in_scratch_tx;
use aether_postgres::platform::PostgresOperatorRepository;
use chrono::Utc;

mod support;
use support::pool;

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

fn operator(subject: &str, rights: PlatformRights) -> PlatformOperator {
    PlatformOperator {
        subject: subject.to_string(),
        rights,
        granted_by: Some("whoever".to_string()),
        granted_at: Utc::now(),
    }
}

#[tokio::test]
async fn a_grant_survives_the_round_trip() {
    let pool = pool_or_skip!();

    let read: Result<Option<PlatformOperator>, CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let operators = PostgresOperatorRepository::new(&tx);
            operators
                .grant(operator(
                    "platform-test-a",
                    PlatformRights::of([PlatformRight::ViewEstate, PlatformRight::ActOnTenant]),
                ))
                .await?;

            operators.find("platform-test-a").await
        },
    )
    .await;

    let back = read.expect("the transaction").expect("the operator");

    assert!(back.rights.holds(PlatformRight::ViewEstate));
    assert!(back.rights.holds(PlatformRight::ActOnTenant));
    assert!(!back.rights.holds(PlatformRight::ManageOperators));
    assert_eq!(back.granted_by.as_deref(), Some("whoever"));
}

/// Narrowing and widening are the same call. Two operations that could
/// disagree about the result is how somebody ends up holding the union of
/// every grant they were ever given.
#[tokio::test]
async fn a_second_grant_replaces_the_first_rather_than_adding_to_it() {
    let pool = pool_or_skip!();

    let read: Result<Option<PlatformRights>, CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let operators = PostgresOperatorRepository::new(&tx);
            operators
                .grant(operator("platform-test-b", PlatformRights::everything()))
                .await?;
            operators
                .grant(operator(
                    "platform-test-b",
                    PlatformRights::of([PlatformRight::ViewEstate]),
                ))
                .await?;

            Ok(operators
                .find("platform-test-b")
                .await?
                .map(|held| held.rights))
        },
    )
    .await;

    assert_eq!(
        read.expect("the transaction"),
        Some(PlatformRights::of([PlatformRight::ViewEstate])),
        "the narrower grant is what stands"
    );
}

/// The count the last-administrator rule is built on. If it counted rows
/// rather than holders of the right, an installation with one administrator
/// and four readers would let the administrator step down.
#[tokio::test]
async fn holders_are_counted_by_the_right_they_hold() {
    let pool = pool_or_skip!();

    let counted: Result<(usize, usize), CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let operators = PostgresOperatorRepository::new(&tx);

            operators
                .grant(operator(
                    "platform-test-admin",
                    PlatformRights::of([PlatformRight::ManageOperators]),
                ))
                .await?;
            for reader in 0..4 {
                operators
                    .grant(operator(
                        &format!("platform-test-reader-{reader}"),
                        PlatformRights::of([PlatformRight::ViewEstate]),
                    ))
                    .await?;
            }

            Ok((
                operators.holders_of(PlatformRight::ManageOperators).await?,
                operators.holders_of(PlatformRight::ViewEstate).await?,
            ))
        },
    )
    .await;

    assert_eq!(
        counted.expect("the transaction"),
        (1, 4),
        "one administrator among four readers"
    );
}

#[tokio::test]
async fn a_revoked_operator_holds_nothing_on_the_next_read() {
    let pool = pool_or_skip!();

    let read: Result<Option<PlatformOperator>, CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let operators = PostgresOperatorRepository::new(&tx);
            operators
                .grant(operator("platform-test-c", PlatformRights::everything()))
                .await?;
            operators.revoke("platform-test-c").await?;

            operators.find("platform-test-c").await
        },
    )
    .await;

    assert!(read.expect("the transaction").is_none());
}

/// A right the code does not know about means the column and the domain have
/// drifted. Read back as an empty set it would silently answer "no" to every
/// question about that operator, which reads as a revocation nobody made.
#[tokio::test]
async fn a_right_nobody_recognises_is_refused_rather_than_dropped() {
    let pool = pool_or_skip!();

    let read: Result<Option<PlatformOperator>, CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            {
                let mut guard = tx.lock().await;
                sqlx::query(
                    "INSERT INTO platform_operators (subject, rights) VALUES ($1, ARRAY['root'])",
                )
                .bind("platform-test-drifted")
                .execute(&mut ***guard)
                .await
                .map_err(|e| CoreError::DatabaseError {
                    message: e.to_string(),
                })?;
            }

            PostgresOperatorRepository::new(&tx)
                .find("platform-test-drifted")
                .await
        },
    )
    .await;

    assert!(
        matches!(read, Err(CoreError::UnknownPlatformRight { .. })),
        "a right nobody recognises was read as something"
    );
}

/// A row holding nothing would answer "yes, an operator" to every screen that
/// lists them while answering "no" to every question about what they may do.
/// Refused by the schema, so nothing has to remember not to write it.
#[tokio::test]
async fn the_database_refuses_an_operator_who_holds_nothing() {
    let pool = pool_or_skip!();

    let written: Result<(), CoreError> = in_scratch_tx(
        &pool,
        |e| CoreError::DatabaseError {
            message: e.to_string(),
        },
        async |tx| {
            let mut guard = tx.lock().await;
            sqlx::query(
                "INSERT INTO platform_operators (subject, rights) VALUES ($1, ARRAY[]::TEXT[])",
            )
            .bind("platform-test-empty")
            .execute(&mut ***guard)
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;

            Ok(())
        },
    )
    .await;

    assert!(written.is_err(), "an operator holding nothing was written");
}
