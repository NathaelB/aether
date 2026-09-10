//! Prints the OpenAPI document to stdout.
//!
//! The console's API client is generated from this document, and the obvious
//! way to get it -- `curl` the running server's `/api-docs/openapi.json` --
//! makes regenerating the client depend on a database, a realm and a port. The
//! document does not: `ApiDoc::openapi()` is a pure function of the handler
//! annotations, so it can be printed by a process that connects to nothing.
//!
//! An example rather than a binary because `cargo build` does not build
//! examples, so this stays out of the release image while `cargo check
//! --all-targets` still type-checks it.
//!
//! ```sh
//! SQLX_OFFLINE=true cargo run -q -p aether-api --example dump-openapi
//! ```

use aether_api::openapi::ApiDoc;
use utoipa::OpenApi;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Pretty-printed on purpose: the document is committed alongside the
    // generated client, and a diff on one line is not a diff anyone can read.
    println!("{}", ApiDoc::openapi().to_pretty_json()?);
    Ok(())
}
