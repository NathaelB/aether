//! Getting a database to test against, and refusing to pretend when there
//! is none.
//!
//! These tests skip themselves when `DATABASE_URL` is unset, which is what
//! lets a developer run `cargo test` without a database. It is also how five
//! of them went a whole chantier without ever running while reporting
//! success. In CI that silence is the failure, so `REQUIRE_DATABASE_URL`
//! turns it into one.

use sqlx::{PgPool, postgres::PgPoolOptions};

pub async fn pool() -> Option<PgPool> {
    let url = std::env::var("DATABASE_URL")
        .ok()
        .filter(|url| !url.is_empty());

    let Some(url) = url else {
        assert!(
            std::env::var("REQUIRE_DATABASE_URL").is_err(),
            "REQUIRE_DATABASE_URL is set but DATABASE_URL is not: these tests \
             would have skipped and reported success"
        );
        eprintln!("skipped: DATABASE_URL is not set");
        return None;
    };

    Some(
        PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .expect("DATABASE_URL is set but the database is unreachable"),
    )
}
