//! What the control plane does about its wrapping key when it starts.
//!
//! **Runs only when `KEY_MANAGER_ADDRESS` is set**, and skips loudly
//! otherwise:
//!
//! ```sh
//! KEY_MANAGER_ADDRESS=http://localhost:8200 \
//!   cargo test -p aether-api --test wrapping_key
//! ```

use std::{sync::Arc, time::Duration};

use aether_api::{
    args::{Args, KeyManagerArgs},
    keys::{ensure_wrapping_key, ensure_wrapping_key_within},
};
use aether_core::backups::{keys::KeyName, ports::KeyProvider};
use aether_transit::TransitKeyProvider;
use uuid::Uuid;

fn args_for(key_manager: KeyManagerArgs) -> Arc<Args> {
    Arc::new(Args {
        key_manager,
        ..Args::default()
    })
}

/// A key per run, so one test never decides another.
fn key_manager_args() -> Option<KeyManagerArgs> {
    let address = std::env::var("KEY_MANAGER_ADDRESS").ok()?;

    Some(KeyManagerArgs {
        address,
        key: format!("aether-start-{}", Uuid::new_v4().simple()),
        ..KeyManagerArgs::default()
    })
}

macro_rules! args_or_skip {
    () => {
        match key_manager_args() {
            Some(args) => args,
            None => {
                eprintln!(
                    "skipping: KEY_MANAGER_ADDRESS is not set, see this file's documentation"
                );
                return;
            }
        }
    };
}

#[tokio::test]
async fn starting_up_gives_archives_a_key_to_be_wrapped_with() {
    let key_manager = args_or_skip!();
    let (config, name) = key_manager.config().expect("wrapping is on");

    ensure_wrapping_key(args_for(key_manager)).await;

    // Generating against it is the only proof that counts. A key that exists
    // and cannot produce a data key is exactly the failure this covers.
    let provider = TransitKeyProvider::new(config).unwrap();
    let generated = provider
        .generate_data_key(&name)
        .await
        .expect("the key the control plane ensured cannot generate a data key");

    assert_eq!(generated.plaintext().len(), 32);
    assert_eq!(generated.key().name, name);
}

#[tokio::test]
async fn starting_up_twice_does_not_replace_the_key() {
    let key_manager = args_or_skip!();
    let (config, name) = key_manager.config().expect("wrapping is on");
    let args = args_for(key_manager);

    ensure_wrapping_key(args.clone()).await;
    let provider = TransitKeyProvider::new(config).unwrap();
    let before = provider.generate_data_key(&name).await.unwrap();

    ensure_wrapping_key(args).await;
    let after = provider.generate_data_key(&name).await.unwrap();

    // The rule that matters: a start is not a rotation. A key replaced on
    // every boot would make every archive older than the last restart
    // unreadable.
    assert_eq!(
        before.key().version,
        after.key().version,
        "starting up rotated the key"
    );

    let unwrapped = provider
        .unwrap_data_key(before.key(), before.wrapped())
        .await
        .expect("a key generated before the second start can no longer be unwrapped");

    assert_eq!(unwrapped.expose(), before.plaintext().expose());
}

#[tokio::test]
async fn an_unreachable_key_manager_does_not_stop_the_control_plane() {
    let key_manager = KeyManagerArgs {
        address: "http://127.0.0.1:1".to_string(),
        ..KeyManagerArgs::default()
    };

    // Returning at all is the assertion. This is spawned beside the router.
    ensure_wrapping_key_within(args_for(key_manager), Duration::from_millis(1)).await;
}

#[tokio::test]
async fn an_installation_that_wraps_nothing_starts_anyway() {
    let key_manager = KeyManagerArgs {
        key: String::new(),
        ..KeyManagerArgs::default()
    };

    ensure_wrapping_key(args_for(key_manager)).await;
}

#[test]
fn a_key_name_that_addresses_something_else_is_not_a_key() {
    let key_manager = KeyManagerArgs {
        key: "transit/keys/../../secret".to_string(),
        ..KeyManagerArgs::default()
    };

    assert!(
        key_manager.config().is_none(),
        "a name holding a path separator would address something other than a key"
    );
    assert!(KeyName::new("transit/keys/../../secret").is_err());
}
