//! The key manager adapter, against anything that speaks Transit.
//!
//! Vault and OpenBao both expose it, and it is the same three operations every
//! cloud key manager exposes under different names: wrap a small payload,
//! unwrap it, generate a data key. Running the real interface locally is why
//! this exists rather than a key held in an environment variable. An adapter
//! shaped against a stand-in has to be redesigned at the first real manager,
//! which is precisely when nobody has time for it.

use std::time::Duration;

use aether_domain::backups::{
    keys::{DataKey, Dek, KeyError, KeyName, KeyRef, KeyVersion, ProviderName, WrappedDek},
    ports::{KeyProvider, KeyProviderAdmin},
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use reqwest::{Client, Response, StatusCode};
use serde::Deserialize;
use tracing::{info, warn};

/// What differs between one Transit endpoint and the next.
#[derive(Debug, Clone)]
pub struct TransitConfig {
    /// Where the key manager answers, with no trailing slash.
    pub address: String,

    /// The token every request carries. Dev mode uses a fixed one; a real
    /// installation uses a scoped one that can generate and decrypt data keys
    /// and nothing else.
    pub token: String,

    /// Where the transit engine is mounted. Almost always `transit`, and
    /// configurable because an installation sharing a key manager with
    /// something else does not get to choose.
    pub mount: String,

    /// Which configured manager this is, recorded on every archive. Adding a
    /// customer's own manager later is another instance of this adapter with
    /// another name, not a new code path.
    pub provider: ProviderName,
}

impl TransitConfig {
    pub fn mount(&self) -> &str {
        &self.mount
    }
}

#[derive(Debug, Clone)]
pub struct TransitKeyProvider {
    client: Client,
    config: TransitConfig,
}

/// Bounded, for the same reason the object store's are: a manager that has not
/// answered in this long is not busy, it is somewhere else, and a wrong address
/// should say so in seconds rather than look like a slow network.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// How much key material a data key carries.
///
/// 256 bits, which is what the archive encryption downstream of this expects
/// and what every manager here can produce. Not configurable: a deployment
/// that quietly generated a weaker key than the one the format assumes would
/// be undetectable from the outside.
const DATA_KEY_BITS: u32 = 256;

#[derive(Debug, Deserialize)]
struct DataKeyResponse {
    data: DataKeyPayload,
}

#[derive(Debug, Deserialize)]
struct DataKeyPayload {
    plaintext: String,
    ciphertext: String,
    key_version: u32,
}

#[derive(Debug, Deserialize)]
struct DecryptResponse {
    data: DecryptPayload,
}

#[derive(Debug, Deserialize)]
struct DecryptPayload {
    plaintext: String,
}

#[derive(Debug, Deserialize, Default)]
struct ErrorResponse {
    #[serde(default)]
    errors: Vec<String>,
}

impl TransitKeyProvider {
    pub fn new(config: TransitConfig) -> Result<Self, KeyError> {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(|error| KeyError::ProviderUnavailable {
                reason: error.to_string(),
            })?;

        Ok(Self { client, config })
    }

    pub fn provider(&self) -> &ProviderName {
        &self.config.provider
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{path}", self.config.address.trim_end_matches('/'))
    }

    async fn post(&self, path: &str, body: serde_json::Value) -> Result<Response, KeyError> {
        self.client
            .post(self.url(path))
            .header("X-Vault-Token", &self.config.token)
            .json(&body)
            .send()
            .await
            .map_err(|error| KeyError::ProviderUnavailable {
                reason: error.to_string(),
            })
    }

    /// Turns a refusal into something the caller can act on.
    ///
    /// The one distinction that matters is whether the key is gone. A restore
    /// that cannot find its key and a restore whose manager is down are
    /// different incidents, and the platform is not allowed to report them the
    /// same way.
    async fn refusal(operation: &str, key: &str, response: Response) -> KeyError {
        let status = response.status();
        let body: ErrorResponse = response.json().await.unwrap_or_default();
        let reason = if body.errors.is_empty() {
            status.to_string()
        } else {
            body.errors.join(", ")
        };

        // Transit says this whether the key was never created or was
        // destroyed. Both mean the same thing to a caller holding an archive
        // it can no longer read.
        if reason.contains("encryption key not found") || status == StatusCode::NOT_FOUND {
            return KeyError::KeyUnavailable {
                key: key.to_string(),
                reason,
            };
        }

        KeyError::Refused {
            operation: operation.to_string(),
            reason,
        }
    }
}

impl KeyProvider for TransitKeyProvider {
    async fn generate_data_key(&self, name: &KeyName) -> Result<DataKey, KeyError> {
        let path = format!("v1/{}/datakey/plaintext/{name}", self.config.mount);
        let response = self
            .post(&path, serde_json::json!({ "bits": DATA_KEY_BITS }))
            .await?;

        if !response.status().is_success() {
            return Err(Self::refusal("generate_data_key", name.as_str(), response).await);
        }

        let body: DataKeyResponse =
            response
                .json()
                .await
                .map_err(|error| KeyError::Malformed {
                    reason: error.to_string(),
                })?;

        let plaintext = BASE64
            .decode(body.data.plaintext)
            .map_err(|error| KeyError::Malformed {
                reason: format!("the data key is not base64: {error}"),
            })?;

        Ok(DataKey::new(
            Dek::new(plaintext),
            WrappedDek::new(body.data.ciphertext),
            // Taken from the manager's own answer rather than parsed out of
            // the ciphertext prefix. Both are available and only one of them
            // is a documented field.
            KeyRef::new(
                self.config.provider.clone(),
                name.clone(),
                KeyVersion::new(body.data.key_version),
            ),
        ))
    }

    async fn unwrap_data_key(
        &self,
        key: &KeyRef,
        wrapped: &WrappedDek,
    ) -> Result<Dek, KeyError> {
        let path = format!("v1/{}/decrypt/{}", self.config.mount, key.name);
        let response = self
            .post(&path, serde_json::json!({ "ciphertext": wrapped.as_str() }))
            .await?;

        if !response.status().is_success() {
            return Err(Self::refusal("unwrap_data_key", &key.to_string(), response).await);
        }

        let body: DecryptResponse =
            response
                .json()
                .await
                .map_err(|error| KeyError::Malformed {
                    reason: error.to_string(),
                })?;

        let plaintext = BASE64
            .decode(body.data.plaintext)
            .map_err(|error| KeyError::Malformed {
                reason: format!("the unwrapped key is not base64: {error}"),
            })?;

        Ok(Dek::new(plaintext))
    }
}

impl KeyProviderAdmin for TransitKeyProvider {
    async fn ensure_key(&self, name: &KeyName) -> Result<(), KeyError> {
        self.ensure_engine().await?;

        let path = format!("v1/{}/keys/{name}", self.config.mount);
        let response = self.post(&path, serde_json::json!({})).await?;

        if !response.status().is_success() {
            return Err(Self::refusal("ensure_key", name.as_str(), response).await);
        }

        info!(key = name.as_str(), "the wrapping key is there");
        Ok(())
    }
}

impl TransitKeyProvider {
    /// Mounts the transit engine if it is not mounted.
    ///
    /// Being already mounted comes back as a refusal rather than as a success,
    /// so it is read and discarded here. Treating every refusal as fatal would
    /// mean the second start of every installation fails.
    async fn ensure_engine(&self) -> Result<(), KeyError> {
        let path = "v1/sys/mounts/".to_string() + &self.config.mount;
        let response = self
            .post(&path, serde_json::json!({ "type": "transit" }))
            .await?;

        if response.status().is_success() {
            info!(mount = self.config.mount, "mounted the transit engine");
            return Ok(());
        }

        let status = response.status();
        let body: ErrorResponse = response.json().await.unwrap_or_default();
        let reason = body.errors.join(", ");

        if reason.contains("already in use") || reason.contains("existing mount") {
            return Ok(());
        }

        // Mounting needs more privilege than using, and an installation whose
        // token may generate data keys but not mount engines is a reasonable
        // thing to run. Say so and carry on: ensure_key answers whether the
        // key is actually usable, and that is the question that matters.
        warn!(
            mount = self.config.mount,
            %status,
            %reason,
            "could not mount the transit engine, continuing in case it is already there"
        );
        Ok(())
    }
}
