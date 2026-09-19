//! Answering the one question a drill (#185) exists to ask: does the
//! restored database answer a query.

use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use tracing::info;

use crate::domain::error::GenesisError;
use crate::domain::ports::{BoxFuture, DatabaseProbe};

/// Bounded on purpose: a drill that cannot even connect within this window is
/// a drill that has already failed, and hanging on a socket a corrupted
/// archive's Postgres left half-started would hold the whole run hostage.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct SqlxDatabaseProbe;

impl DatabaseProbe for SqlxDatabaseProbe {
    fn answers_a_query<'a>(&'a self, uri: &'a str) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async move {
            info!("connecting to the restored database");

            let pool = PgPoolOptions::new()
                .max_connections(1)
                .acquire_timeout(CONNECT_TIMEOUT)
                .connect(uri)
                .await
                .map_err(|error| GenesisError::Internal {
                    message: format!("the restored database refused a connection: {error}"),
                })?;

            let answer: (i32,) =
                sqlx::query_as("SELECT 1")
                    .fetch_one(&pool)
                    .await
                    .map_err(|error| GenesisError::Internal {
                        message: format!("the restored database did not answer: {error}"),
                    })?;

            pool.close().await;

            if answer.0 != 1 {
                // Unreachable in practice -- `SELECT 1` has one answer -- but
                // a probe that trusts the connection without reading the
                // response is a probe that would pass against a database that
                // opened a socket and returned garbage.
                return Err(GenesisError::Internal {
                    message: "the restored database answered something other than 1".to_string(),
                });
            }

            Ok(())
        })
    }
}
