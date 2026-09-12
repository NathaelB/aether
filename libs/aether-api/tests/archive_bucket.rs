//! What the control plane does about the archive bucket when it starts.
//!
//! Needs a real store, because what is worth covering is the behaviour against
//! one that is there, one that is not, and one that already holds the bucket.
//! A mock would only assert that a method was called.
//!
//! **Runs only when `OBJECT_STORE_ENDPOINT` is set**, and skips loudly
//! otherwise:
//!
//! ```sh
//! OBJECT_STORE_ENDPOINT=http://localhost:9800 \
//!   cargo test -p aether-api --test archive_bucket
//! ```

use std::{sync::Arc, time::Duration};

use aether_api::{
    args::{Args, ObjectStoreArgs},
    objectstore::{ensure_archive_bucket, ensure_archive_bucket_within},
};
use aether_core::{
    backups::{ArchivePrefix, ports::BackupStore},
    deployments::DeploymentId,
    organisation::OrganisationId,
};
use aether_s3::{ObjectStoreConfig, S3ObjectStore};
use uuid::Uuid;

fn args_for(object_store: ObjectStoreArgs) -> Arc<Args> {
    Arc::new(Args {
        object_store,
        ..Args::default()
    })
}

/// A bucket name per run, so one failed test never decides the next one.
fn object_store_args() -> Option<ObjectStoreArgs> {
    let endpoint = std::env::var("OBJECT_STORE_ENDPOINT").ok()?;

    Some(ObjectStoreArgs {
        endpoint: Some(endpoint),
        bucket: format!("aether-start-{}", Uuid::new_v4().simple()),
        ..ObjectStoreArgs::default()
    })
}

macro_rules! args_or_skip {
    () => {
        match object_store_args() {
            Some(args) => args,
            None => {
                eprintln!(
                    "skipping: OBJECT_STORE_ENDPOINT is not set, see this file's documentation"
                );
                return;
            }
        }
    };
}

fn store_for(args: &ObjectStoreArgs) -> S3ObjectStore {
    let config: ObjectStoreConfig = args
        .config()
        .expect("archiving is on")
        .expect("the bucket name is valid");

    S3ObjectStore::new(config)
}

#[tokio::test]
async fn starting_up_gives_archives_somewhere_to_go() {
    let object_store = args_or_skip!();
    let store = store_for(&object_store);

    ensure_archive_bucket(args_for(object_store)).await;

    // Writing into it is the only proof that counts. A bucket that exists and
    // refuses writes is exactly the failure this test is here for.
    let prefix = ArchivePrefix::new(OrganisationId(Uuid::new_v4()), DeploymentId(Uuid::new_v4()));
    let at = prefix.object("manifest.json").unwrap();

    store
        .put(&at, b"{}".to_vec(), "application/json")
        .await
        .expect("the bucket the control plane ensured does not accept writes");

    assert_eq!(store.get(&at).await.unwrap(), Some(b"{}".to_vec()));
}

#[tokio::test]
async fn starting_up_twice_changes_nothing() {
    let object_store = args_or_skip!();
    let args = args_for(object_store.clone());

    ensure_archive_bucket(args.clone()).await;
    ensure_archive_bucket(args).await;

    let store = store_for(&object_store);
    let prefix = ArchivePrefix::new(OrganisationId(Uuid::new_v4()), DeploymentId(Uuid::new_v4()));
    let at = prefix.object("manifest.json").unwrap();

    store
        .put(&at, b"{}".to_vec(), "application/json")
        .await
        .expect("the bucket stopped being usable after a second start");
}

#[tokio::test]
async fn an_unreachable_store_does_not_stop_the_control_plane() {
    let object_store = ObjectStoreArgs {
        // Port 1 on the loopback, where the refusal is immediate. A routable
        // address that drops packets would test the SDK's connect timeout
        // instead, which is not what this covers and costs a minute to learn.
        endpoint: Some("http://127.0.0.1:1".to_string()),
        bucket: "aether-unreachable".to_string(),
        ..ObjectStoreArgs::default()
    };

    // Returning at all is the assertion. This is spawned beside the router, so
    // a version of it that blocks for ever takes the control plane with it.
    ensure_archive_bucket_within(args_for(object_store), Duration::from_millis(1)).await;
}

#[tokio::test]
async fn an_installation_that_archives_nowhere_starts_anyway() {
    let object_store = ObjectStoreArgs {
        bucket: String::new(),
        ..ObjectStoreArgs::default()
    };

    ensure_archive_bucket(args_for(object_store)).await;
}
