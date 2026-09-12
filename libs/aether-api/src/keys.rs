//! Making sure the key archives are wrapped with exists.
//!
//! Same shape as the archive bucket next door, and for the same reason: an
//! installation whose key manager was rebuilt comes back without anybody
//! noticing, and one that wraps with nothing says so on the first line of its
//! logs rather than at the first restore.
//!
//! This never destroys and never rotates. Creating a key that already exists
//! is a no-op on every manager here, which is what makes calling it on every
//! start safe. Rotation is an operator action, and a platform that rotated on
//! its own would be deciding for a customer when their archives change key.

use std::{sync::Arc, time::Duration};

use aether_core::backups::{
    keys::{KeyName, ProviderName},
    ports::KeyProviderAdmin,
};
use aether_transit::{TransitConfig, TransitKeyProvider};
use tracing::{error, info, warn};

use crate::args::{Args, KeyManagerArgs};

/// Matches the object store's, because both are racing the same Compose start.
const STARTUP_GRACE: Duration = Duration::from_secs(30);
const RETRY_INTERVAL: Duration = Duration::from_secs(3);

impl KeyManagerArgs {
    /// `None` when this installation wraps nothing, which is what an empty key
    /// name means.
    pub fn config(&self) -> Option<(TransitConfig, KeyName)> {
        if self.key.trim().is_empty() {
            return None;
        }

        let name = KeyName::new(self.key.clone()).ok()?;

        Some((
            TransitConfig {
                address: self.address.clone(),
                token: self.token.clone(),
                mount: self.mount.clone(),
                provider: ProviderName::platform(),
            },
            name,
        ))
    }
}

pub async fn ensure_wrapping_key(args: Arc<Args>) {
    ensure_wrapping_key_within(args, STARTUP_GRACE).await
}

/// [`ensure_wrapping_key`] with the retry window spelled out, so a test can
/// assert what happens when the manager never comes up without waiting the
/// real grace to find out.
pub async fn ensure_wrapping_key_within(args: Arc<Args>, grace: Duration) {
    let Some((config, name)) = args.key_manager.config() else {
        warn!("no wrapping key is configured: this installation wraps nothing");
        return;
    };

    let provider = match TransitKeyProvider::new(config) {
        Ok(provider) => provider,
        Err(error) => {
            error!(%error, "the key manager client could not be built");
            return;
        }
    };

    let deadline = tokio::time::Instant::now() + grace;

    loop {
        match provider.ensure_key(&name).await {
            Ok(()) => {
                info!(
                    key = name.as_str(),
                    "archives have a key to be wrapped with"
                );
                return;
            }
            Err(error) if tokio::time::Instant::now() < deadline => {
                info!(key = name.as_str(), %error, "the key manager is not up yet, trying again");
                tokio::time::sleep(RETRY_INTERVAL).await;
            }
            Err(error) => {
                // Loud, and not fatal. A key manager that is down stops
                // backups, not authentication, and the alternative to finding
                // out here is finding out during a restore.
                error!(
                    key = name.as_str(),
                    %error,
                    "the wrapping key could not be reached: nothing will be backed up until it can"
                );
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_key_name_means_this_installation_wraps_nothing() {
        let args = KeyManagerArgs {
            key: "   ".to_string(),
            ..KeyManagerArgs::default()
        };

        assert!(args.config().is_none());
    }

    #[test]
    fn the_default_points_at_the_key_manager_in_compose() {
        let (config, name) = KeyManagerArgs::default().config().expect("wrapping is on");

        assert_eq!(config.mount(), "transit");
        assert_eq!(name.as_str(), "aether-backups");
        assert_eq!(config.provider, ProviderName::platform());
    }
}
