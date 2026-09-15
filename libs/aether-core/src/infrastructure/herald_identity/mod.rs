//! Minting a data plane's identity in FerrisKey.
//!
//! The one administrative act the control plane performs on the realm, and the
//! only place it holds credentials that can create one. Kept behind
//! [`HeraldIdentityProvisioner`] so the privilege is visible in the wiring
//! rather than reachable from anywhere a request is handled.
//!
//! Three things about the API this talks to, each learned by asking it:
//!
//! * Creating a client needs both `client_type` and `protocol`, and the error
//!   for a missing field names one at a time.
//! * The creation response carries `client_secret` in the clear. It is the
//!   only time it is readable without a second call, and the only time this
//!   platform ever sees it.
//! * A client's tokens carry the subject of its *service account*, which is a
//!   different id from the client's own. It is resolved from the realm's users
//!   by the name FerrisKey gives it -- the one place that convention is relied
//!   on, and it fails loudly here rather than silently at authorisation time.

use aether_domain::{
    CoreError,
    dataplane::{
        herald_identity::{HeraldBinding, MintedHeraldIdentity},
        ports::HeraldIdentityProvisioner,
        value_objects::DataPlaneId,
    },
};
use reqwest::Client;
use serde::Deserialize;
use tracing::{info, warn};

/// Where the realm is, and who may administer it.
#[derive(Debug, Clone)]
pub struct RealmAdmin {
    /// The identity provider's base url, without the realm.
    pub base_url: String,
    pub realm: String,

    /// The realm the administrator itself lives in, which is not the one being
    /// administered: FerrisKey's admin account is in `master`.
    pub admin_realm: String,
    pub admin_client_id: String,
    pub admin_username: String,
    pub admin_password: String,
}

impl RealmAdmin {
    /// `None` when this installation was not given an administrator, which is
    /// every installation that has not been reconfigured since data planes
    /// gained identities of their own.
    pub fn configured(self) -> Option<Self> {
        if self.admin_username.trim().is_empty() || self.base_url.trim().is_empty() {
            return None;
        }

        Some(self)
    }
}

pub struct FerrisKeyHeraldIdentities {
    http: Client,
    realm: RealmAdmin,
}

#[derive(Deserialize)]
struct TokenAnswer {
    access_token: String,
}

#[derive(Deserialize)]
struct ClientAnswer {
    client_secret: Option<String>,
}

#[derive(Deserialize)]
struct Listed<T> {
    data: Vec<T>,
}

#[derive(Deserialize)]
struct RealmUser {
    id: String,
    username: String,
}

impl FerrisKeyHeraldIdentities {
    pub fn new(realm: RealmAdmin) -> Self {
        Self {
            http: Client::new(),
            realm,
        }
    }

    fn failed(what: &str, detail: impl std::fmt::Display) -> CoreError {
        CoreError::InternalError(format!("could not {what} in the realm: {detail}"))
    }

    /// An administrator's token, obtained per call rather than cached.
    ///
    /// These calls happen when a data plane is registered, which is rare. A
    /// cache would be a held credential and a lifetime to get wrong, bought
    /// for a round trip nobody is waiting on.
    async fn admin_token(&self) -> Result<String, CoreError> {
        let answer = self
            .http
            .post(format!(
                "{}/realms/{}/protocol/openid-connect/token",
                self.realm.base_url, self.realm.admin_realm
            ))
            .form(&[
                ("grant_type", "password"),
                ("client_id", self.realm.admin_client_id.as_str()),
                ("username", self.realm.admin_username.as_str()),
                ("password", self.realm.admin_password.as_str()),
            ])
            .send()
            .await
            .map_err(|e| Self::failed("reach the identity provider", e))?;

        if !answer.status().is_success() {
            return Err(Self::failed(
                "authenticate as the realm administrator",
                answer.status(),
            ));
        }

        Ok(answer
            .json::<TokenAnswer>()
            .await
            .map_err(|e| Self::failed("read the administrator's token", e))?
            .access_token)
    }

    async fn find_client(&self, token: &str, client_id: &str) -> Result<Option<String>, CoreError> {
        let answer = self
            .http
            .get(format!(
                "{}/realms/{}/clients",
                self.realm.base_url, self.realm.realm
            ))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| Self::failed("list the realm's clients", e))?;

        if !answer.status().is_success() {
            return Err(Self::failed("list the realm's clients", answer.status()));
        }

        #[derive(Deserialize)]
        struct ExistingClient {
            id: String,
            client_id: String,
        }

        let listed: Listed<ExistingClient> = answer
            .json()
            .await
            .map_err(|e| Self::failed("read the realm's clients", e))?;

        Ok(listed
            .data
            .into_iter()
            .find(|client| client.client_id == client_id)
            .map(|client| client.id))
    }

    async fn delete_client(&self, token: &str, uuid: &str) -> Result<(), CoreError> {
        let answer = self
            .http
            .delete(format!(
                "{}/realms/{}/clients/{uuid}",
                self.realm.base_url, self.realm.realm
            ))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| Self::failed("remove a client", e))?;

        if !answer.status().is_success() {
            return Err(Self::failed("remove a client", answer.status()));
        }

        Ok(())
    }

    /// The subject a client's tokens will carry.
    ///
    /// Resolved by the name FerrisKey gives a client's service account. It is
    /// the one place that naming convention is trusted, and it is trusted here
    /// -- once, at registration, where failing is loud -- rather than on every
    /// request, where a change to it would lock every data plane out at the
    /// same moment.
    async fn service_account_of(&self, token: &str, client_id: &str) -> Result<String, CoreError> {
        let answer = self
            .http
            .get(format!(
                "{}/realms/{}/users",
                self.realm.base_url, self.realm.realm
            ))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| Self::failed("list the realm's users", e))?;

        if !answer.status().is_success() {
            return Err(Self::failed("list the realm's users", answer.status()));
        }

        let listed: Listed<RealmUser> = answer
            .json()
            .await
            .map_err(|e| Self::failed("read the realm's users", e))?;

        let expected = format!("service-account-{client_id}");

        listed
            .data
            .into_iter()
            .find(|user| user.username == expected)
            .map(|user| user.id)
            .ok_or_else(|| {
                Self::failed(
                    "find the service account of a client just created",
                    format!("no user called {expected}"),
                )
            })
    }
}

impl HeraldIdentityProvisioner for FerrisKeyHeraldIdentities {
    async fn mint(&self, dataplane: DataPlaneId) -> Result<MintedHeraldIdentity, CoreError> {
        let token = self.admin_token().await?;
        let client_id = MintedHeraldIdentity::client_id_for(dataplane);

        // Removed first when it is already there, rather than updated. The
        // secret is only readable in a creation response, so rotating means
        // creating again -- and a client left behind would be a credential
        // nobody knows about that still authenticates.
        if let Some(existing) = self.find_client(&token, &client_id).await? {
            info!(%client_id, "replacing the existing client");
            self.delete_client(&token, &existing).await?;
        }

        let answer = self
            .http
            .post(format!(
                "{}/realms/{}/clients",
                self.realm.base_url, self.realm.realm
            ))
            .bearer_auth(&token)
            .json(&serde_json::json!({
                "client_id": client_id,
                "name": format!("Herald for data plane {}", dataplane.0),
                "enabled": true,
                // Both are required, and the error names one missing field at
                // a time -- so leaving either out costs two round trips to
                // find out.
                "client_type": "confidential",
                "protocol": "openid-connect",
                "public_client": false,
                "service_account_enabled": true,
                // A data plane authenticates as itself and never on behalf of
                // somebody: nothing here should be able to take a password.
                "direct_access_grants_enabled": false,
            }))
            .send()
            .await
            .map_err(|e| Self::failed("create a client", e))?;

        if !answer.status().is_success() {
            return Err(Self::failed("create a client", answer.status()));
        }

        let created: ClientAnswer = answer
            .json()
            .await
            .map_err(|e| Self::failed("read the client just created", e))?;

        let secret = created.client_secret.ok_or_else(|| {
            Self::failed(
                "read the secret of a client just created",
                "the response carried none",
            )
        })?;

        let subject = self.service_account_of(&token, &client_id).await?;

        info!(%client_id, "minted an identity for a data plane");

        Ok(MintedHeraldIdentity {
            binding: HeraldBinding { client_id, subject },
            secret,
        })
    }

    async fn revoke(&self, dataplane: DataPlaneId) -> Result<(), CoreError> {
        let token = self.admin_token().await?;
        let client_id = MintedHeraldIdentity::client_id_for(dataplane);

        match self.find_client(&token, &client_id).await? {
            Some(uuid) => self.delete_client(&token, &uuid).await,
            None => {
                // Already gone is the outcome asked for. Said rather than
                // silent, because it also means somebody removed it by hand.
                warn!(%client_id, "no client to revoke");
                Ok(())
            }
        }
    }
}
