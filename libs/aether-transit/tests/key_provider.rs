//! The Transit adapter against a real key manager.
//!
//! Everything worth covering here is behaviour the manager owns: what a
//! rotation does to an archive encrypted before it, what a destroyed key looks
//! like to a caller, and whether an unreachable manager is distinguishable
//! from a missing key. A mock answers all three whichever way it was told to.
//!
//! **Runs only when `KEY_MANAGER_ADDRESS` is set**, and skips loudly
//! otherwise:
//!
//! ```sh
//! KEY_MANAGER_ADDRESS=http://localhost:8200 KEY_MANAGER_TOKEN=aether-root \
//!   cargo test -p aether-transit --test key_provider
//! ```

use aether_domain::backups::{
    keys::{KeyError, KeyName, KeyRef, KeyVersion, ProviderName, WrappedDek},
    ports::{KeyProvider, KeyProviderAdmin},
};
use aether_transit::{TransitConfig, TransitKeyProvider};
use uuid::Uuid;

fn provider() -> Option<TransitKeyProvider> {
    let address = std::env::var("KEY_MANAGER_ADDRESS").ok()?;

    let config = TransitConfig {
        address,
        token: std::env::var("KEY_MANAGER_TOKEN").unwrap_or_else(|_| "aether-root".to_string()),
        mount: "transit".to_string(),
        provider: ProviderName::platform(),
    };

    Some(TransitKeyProvider::new(config).expect("the client is buildable"))
}

macro_rules! provider_or_skip {
    () => {
        match provider() {
            Some(provider) => provider,
            None => {
                eprintln!(
                    "skipping: KEY_MANAGER_ADDRESS is not set, see this file's documentation"
                );
                return;
            }
        }
    };
}

/// A key per test, so a rotation in one never decides another.
fn key_name() -> KeyName {
    KeyName::new(format!("aether-test-{}", Uuid::new_v4().simple())).unwrap()
}

#[tokio::test]
async fn a_generated_key_comes_back_from_what_was_stored_beside_it() {
    let provider = provider_or_skip!();
    let name = key_name();
    provider.ensure_key(&name).await.unwrap();

    let generated = provider.generate_data_key(&name).await.unwrap();
    let (plaintext, wrapped, reference) = generated.into_parts();

    assert_eq!(plaintext.len(), 32, "a data key is 256 bits");
    assert_eq!(reference.name, name);
    assert_eq!(reference.provider, ProviderName::platform());

    let unwrapped = provider
        .unwrap_data_key(&reference, &wrapped)
        .await
        .unwrap();

    assert_eq!(unwrapped.expose(), plaintext.expose());
}

#[tokio::test]
async fn a_rotation_does_not_rewrite_what_came_before_it() {
    let provider = provider_or_skip!();
    let name = key_name();
    provider.ensure_key(&name).await.unwrap();

    let before = provider.generate_data_key(&name).await.unwrap();
    let (plaintext, wrapped, reference) = before.into_parts();
    assert_eq!(reference.version, KeyVersion::new(1));

    rotate(&name).await;

    // The rule the whole module exists for. An archive taken before a rotation
    // stays readable, and it stays readable under the version it recorded.
    let after = provider.generate_data_key(&name).await.unwrap();
    assert_eq!(
        after.key().version,
        KeyVersion::new(2),
        "the rotation did not produce a new version"
    );

    let unwrapped = provider
        .unwrap_data_key(&reference, &wrapped)
        .await
        .unwrap();

    assert_eq!(
        unwrapped.expose(),
        plaintext.expose(),
        "an archive taken before the rotation can no longer be read"
    );
}

#[tokio::test]
async fn a_key_that_is_gone_says_so_rather_than_failing_vaguely() {
    let provider = provider_or_skip!();
    let name = key_name();

    // Never created, which is what a destroyed key looks like to a caller
    // holding an archive that names it.
    let error = provider.generate_data_key(&name).await.unwrap_err();

    assert!(
        matches!(error, KeyError::KeyUnavailable { .. }),
        "a missing key has to be distinguishable from every other failure, got {error:?}"
    );
}

#[tokio::test]
async fn an_unreachable_manager_is_not_a_missing_key() {
    let config = TransitConfig {
        // Port 1 on the loopback: refused immediately, so this measures the
        // classification rather than a connect timeout.
        address: "http://127.0.0.1:1".to_string(),
        token: "irrelevant".to_string(),
        mount: "transit".to_string(),
        provider: ProviderName::platform(),
    };
    let provider = TransitKeyProvider::new(config).unwrap();

    // Matched rather than unwrapped: `Dek` has no `Debug` on purpose, which is
    // what stops key material reaching a panic message or a log line, and
    // `unwrap_err` needs one.
    let result = provider
        .unwrap_data_key(
            &KeyRef::new(
                ProviderName::platform(),
                KeyName::new("aether-backups").unwrap(),
                KeyVersion::new(1),
            ),
            &WrappedDek::new("vault:v1:whatever"),
        )
        .await;
    let error = match result {
        Ok(_) => panic!("an unreachable key manager unwrapped a key"),
        Err(error) => error,
    };

    assert!(
        matches!(error, KeyError::ProviderUnavailable { .. }),
        "a manager that is down was reported as something else, got {error:?}"
    );
}

#[tokio::test]
async fn ensuring_a_key_twice_is_not_an_error() {
    let provider = provider_or_skip!();
    let name = key_name();

    provider.ensure_key(&name).await.unwrap();
    provider.ensure_key(&name).await.unwrap();

    provider.generate_data_key(&name).await.unwrap();
}

/// Rotation is an operator action rather than something the platform does, so
/// it is not on the port. The test drives it the way an operator would.
async fn rotate(name: &KeyName) {
    let address = std::env::var("KEY_MANAGER_ADDRESS").unwrap();
    let token = std::env::var("KEY_MANAGER_TOKEN").unwrap_or_else(|_| "aether-root".to_string());

    let response = reqwest::Client::new()
        .post(format!("{address}/v1/transit/keys/{name}/rotate"))
        .header("X-Vault-Token", token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("the key manager is reachable");

    assert!(response.status().is_success(), "the rotation was refused");
}
