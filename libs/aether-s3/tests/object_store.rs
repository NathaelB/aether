//! The S3 adapter against a real store rather than a mock.
//!
//! A mock of an object store asserts that a string was sent, which is the one
//! thing that was never in doubt. What is in doubt is whether path style
//! addressing, bucket creation, absence of a key and prefix listing behave the
//! way the port promises, and only an implementation can answer that.
//!
//! **Runs only when `OBJECT_STORE_ENDPOINT` is set**, and skips loudly
//! otherwise. There is no object store in CI yet, so making it mandatory would
//! turn every run red. With one:
//!
//! ```sh
//! OBJECT_STORE_ENDPOINT=http://localhost:9800 \
//!   cargo test -p aether-s3 --test object_store
//! ```

use aether_domain::{
    backups::{
        ArchivePrefix, BucketName,
        ports::{BackupStore, BackupStoreAdmin},
    },
    deployments::DeploymentId,
    organisation::OrganisationId,
};
use aether_s3::{ObjectStoreConfig, S3ObjectStore};
use uuid::Uuid;

/// A bucket per run, so a failed test never poisons the next one and the
/// suite can run against a store holding real buckets.
fn store() -> Option<(S3ObjectStore, BucketName)> {
    let endpoint = std::env::var("OBJECT_STORE_ENDPOINT").ok()?;
    let bucket = BucketName::new(format!("aether-test-{}", Uuid::new_v4().simple())).unwrap();

    let config = ObjectStoreConfig {
        endpoint: Some(endpoint),
        region: "us-east-1".to_string(),
        access_key_id: std::env::var("OBJECT_STORE_ACCESS_KEY")
            .unwrap_or_else(|_| "aether".to_string()),
        secret_access_key: std::env::var("OBJECT_STORE_SECRET_KEY")
            .unwrap_or_else(|_| "aetheraether".to_string()),
        force_path_style: true,
        bucket: bucket.clone(),
    };

    Some((S3ObjectStore::new(config), bucket))
}

macro_rules! store_or_skip {
    () => {
        match store() {
            Some(pair) => pair,
            None => {
                eprintln!(
                    "skipping: OBJECT_STORE_ENDPOINT is not set, see this file's documentation"
                );
                return;
            }
        }
    };
}

fn prefix() -> ArchivePrefix {
    ArchivePrefix::new(OrganisationId(Uuid::new_v4()), DeploymentId(Uuid::new_v4()))
}

#[tokio::test]
async fn an_object_written_comes_back_unchanged() {
    let (store, bucket) = store_or_skip!();
    store.ensure_bucket(&bucket).await.unwrap();

    let prefix = prefix();
    let at = prefix.object("manifest.json").unwrap();
    let body = br#"{"version":"26.0.0"}"#.to_vec();

    store
        .put(&at, body.clone(), "application/json")
        .await
        .unwrap();

    assert_eq!(store.get(&at).await.unwrap(), Some(body));
}

#[tokio::test]
async fn an_object_that_is_not_there_is_absent_rather_than_an_error() {
    let (store, bucket) = store_or_skip!();
    store.ensure_bucket(&bucket).await.unwrap();

    let at = prefix().object("never-written.json").unwrap();

    assert_eq!(store.get(&at).await.unwrap(), None);
}

#[tokio::test]
async fn a_listing_stops_at_the_prefix_it_was_given() {
    let (store, bucket) = store_or_skip!();
    store.ensure_bucket(&bucket).await.unwrap();

    let organisation = OrganisationId(Uuid::new_v4());
    let mine = ArchivePrefix::new(organisation, DeploymentId(Uuid::new_v4()));
    let theirs = ArchivePrefix::new(organisation, DeploymentId(Uuid::new_v4()));

    store
        .put(
            &mine.object("base/one").unwrap(),
            b"mine".to_vec(),
            "application/octet-stream",
        )
        .await
        .unwrap();
    store
        .put(
            &theirs.object("base/one").unwrap(),
            b"theirs".to_vec(),
            "application/octet-stream",
        )
        .await
        .unwrap();

    let listed = store.list(&mine).await.unwrap();

    assert_eq!(listed.len(), 1, "listing crossed into another deployment");
    assert_eq!(listed[0].key().as_str(), "base/one");
    assert_eq!(listed[0].prefix(), &mine);
}

#[tokio::test]
async fn a_deleted_object_is_gone() {
    let (store, bucket) = store_or_skip!();
    store.ensure_bucket(&bucket).await.unwrap();

    let at = prefix().object("transient").unwrap();
    store
        .put(&at, b"here".to_vec(), "application/octet-stream")
        .await
        .unwrap();

    store.delete(&at).await.unwrap();

    assert_eq!(store.get(&at).await.unwrap(), None);
}

#[tokio::test]
async fn ensuring_a_bucket_twice_is_not_an_error() {
    let (store, bucket) = store_or_skip!();

    store.ensure_bucket(&bucket).await.unwrap();
    store.ensure_bucket(&bucket).await.unwrap();
}

#[tokio::test]
async fn a_bucket_discards_the_parts_of_uploads_that_never_finished() {
    let (store, bucket) = store_or_skip!();
    store.ensure_bucket(&bucket).await.unwrap();

    let applied = store
        .client()
        .get_bucket_lifecycle_configuration()
        .bucket(bucket.as_str())
        .send()
        .await
        .expect("the store accepted the rule when the bucket was ensured, so it can be read back");

    let rule = applied
        .rules()
        .iter()
        .find(|rule| rule.id() == Some("abort-incomplete-multipart-uploads"))
        .expect("the rule every bucket this platform writes to must carry is missing");

    assert_eq!(
        rule.abort_incomplete_multipart_upload()
            .and_then(|abort| abort.days_after_initiation()),
        Some(aether_domain::backups::INCOMPLETE_UPLOAD_GRACE_DAYS as i32)
    );
}
