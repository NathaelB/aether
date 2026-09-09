use std::time::{Duration, Instant};

use reqwest::Client;
use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::debug;

use crate::domain::error::HeraldError;

/// Refresh this long before a token actually expires.
///
/// The control plane checks `exp` against its own clock, so a token that looks
/// valid here can already be rejected there. A margin costs one extra token
/// request per lifetime and removes a class of failure that would otherwise
/// appear as an unexplained 401 in the middle of a sync cycle.
const REFRESH_MARGIN: Duration = Duration::from_secs(60);

/// Used when the token response carries no `expires_in`. Short on purpose: a
/// wrong guess that is too long fails closed at the worst moment, one that is
/// too short costs a request.
const FALLBACK_LIFETIME: Duration = Duration::from_secs(300);

/// How Herald proves it is `herald-service` to the control plane.
pub enum ControlPlaneAuth {
    /// A fixed token.
    ///
    /// Fine for a test or a short local run, and wrong for anything that runs
    /// longer than the token lives: the control plane does check `exp`, so a
    /// static token stops working with no way for Herald to recover.
    Static(String),

    /// OAuth2 client credentials against the identity provider, cached and
    /// refreshed before expiry.
    ClientCredentials {
        client: Client,
        token_url: String,
        client_id: String,
        client_secret: String,
        cached: RwLock<Option<CachedToken>>,
    },
}

pub struct CachedToken {
    value: String,
    /// When this token stops being usable, margin already subtracted.
    usable_until: Instant,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: Option<u64>,
}

impl ControlPlaneAuth {
    /// Builds a client-credentials source from an OIDC issuer, deriving the
    /// token endpoint the way Keycloak and Ferriskey expose it.
    pub fn client_credentials(
        issuer: impl AsRef<str>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Self {
        let issuer = issuer.as_ref().trim_end_matches('/');

        Self::ClientCredentials {
            client: Client::new(),
            token_url: format!("{issuer}/protocol/openid-connect/token"),
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            cached: RwLock::new(None),
        }
    }

    /// Returns a bearer token, fetching or refreshing one if needed.
    pub async fn bearer(&self) -> Result<String, HeraldError> {
        match self {
            Self::Static(token) => Ok(token.clone()),
            Self::ClientCredentials { cached, .. } => {
                if let Some(token) = cached
                    .read()
                    .await
                    .as_ref()
                    .filter(|token| Instant::now() < token.usable_until)
                {
                    return Ok(token.value.clone());
                }

                self.refresh().await
            }
        }
    }

    async fn refresh(&self) -> Result<String, HeraldError> {
        let Self::ClientCredentials {
            client,
            token_url,
            client_id,
            client_secret,
            cached,
        } = self
        else {
            unreachable!("refresh is only reachable for client credentials");
        };

        // Re-checked under the write lock: several sync cycles can find the
        // token expired at once, and without this they would each mint a new
        // one.
        let mut guard = cached.write().await;
        if let Some(token) = guard
            .as_ref()
            .filter(|token| Instant::now() < token.usable_until)
        {
            return Ok(token.value.clone());
        }

        let response = client
            .post(token_url)
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
            ])
            .send()
            .await
            .map_err(|e| HeraldError::ControlPlane {
                // Deliberately not `{e:?}` and never the form body: a client
                // secret in a log line outlives the incident that produced it.
                message: format!("token request to {token_url} failed: {e}"),
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(HeraldError::ControlPlane {
                message: format!("token request to {token_url} returned {status}"),
            });
        }

        let body: TokenResponse = response
            .json()
            .await
            .map_err(|e| HeraldError::ControlPlane {
                message: format!("token response was not readable: {e}"),
            })?;

        let lifetime = body
            .expires_in
            .map(Duration::from_secs)
            .unwrap_or(FALLBACK_LIFETIME);

        // saturating_sub: a token that lives less than the margin is used
        // immediately rather than being treated as already expired, which
        // would loop.
        let usable_for = lifetime.saturating_sub(REFRESH_MARGIN);

        debug!(
            lifetime_seconds = lifetime.as_secs(),
            usable_for_seconds = usable_for.as_secs(),
            "refreshed the control plane token"
        );

        let token = body.access_token;
        *guard = Some(CachedToken {
            value: token.clone(),
            usable_until: Instant::now() + usable_for,
        });

        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    fn token_body(access_token: &str, expires_in: Option<u64>) -> String {
        match expires_in {
            Some(seconds) => format!(
                r#"{{"access_token":"{access_token}","token_type":"Bearer","expires_in":{seconds}}}"#
            ),
            None => format!(r#"{{"access_token":"{access_token}","token_type":"Bearer"}}"#),
        }
    }

    fn auth_for(server: &MockServer) -> ControlPlaneAuth {
        ControlPlaneAuth::client_credentials(server.base_url(), "herald-service", "s3cret")
    }

    #[tokio::test]
    async fn the_token_endpoint_follows_the_oidc_convention() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/protocol/openid-connect/token")
                    .body_contains("grant_type=client_credentials")
                    .body_contains("client_id=herald-service");
                then.status(200)
                    .header("content-type", "application/json")
                    .body(token_body("first", Some(3600)));
            })
            .await;

        let token = auth_for(&server).bearer().await.expect("a token");

        assert_eq!(token, "first");
        mock.assert_async().await;
    }

    /// One request per token lifetime, not one per call. Herald asks for a
    /// bearer on every control plane request, so a provider that did not cache
    /// would mint a token per HTTP call.
    #[tokio::test]
    async fn a_valid_token_is_reused_rather_than_refetched() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST).path("/protocol/openid-connect/token");
                then.status(200)
                    .header("content-type", "application/json")
                    .body(token_body("cached", Some(3600)));
            })
            .await;

        let auth = auth_for(&server);
        for _ in 0..5 {
            assert_eq!(auth.bearer().await.expect("a token"), "cached");
        }

        mock.assert_hits_async(1).await;
    }

    /// A lifetime shorter than the refresh margin leaves no usable window, so
    /// every call refreshes. That is the degenerate case of refreshing *before*
    /// expiry rather than after a 401, and it must not deadlock or loop.
    #[tokio::test]
    async fn a_token_inside_the_refresh_margin_is_refreshed() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST).path("/protocol/openid-connect/token");
                then.status(200)
                    .header("content-type", "application/json")
                    .body(token_body("short-lived", Some(1)));
            })
            .await;

        let auth = auth_for(&server);
        assert_eq!(auth.bearer().await.expect("a token"), "short-lived");
        assert_eq!(auth.bearer().await.expect("a token"), "short-lived");

        mock.assert_hits_async(2).await;
    }

    #[tokio::test]
    async fn a_response_without_expires_in_still_caches() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST).path("/protocol/openid-connect/token");
                then.status(200)
                    .header("content-type", "application/json")
                    .body(token_body("no-expiry", None));
            })
            .await;

        let auth = auth_for(&server);
        auth.bearer().await.expect("a token");
        auth.bearer().await.expect("a token");

        mock.assert_hits_async(1).await;
    }

    /// The secret is sent in the form body and must never come back out in an
    /// error: a credential in a log line outlives the incident that produced it.
    #[tokio::test]
    async fn a_rejected_token_request_does_not_leak_the_secret() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/protocol/openid-connect/token");
                then.status(401).body("invalid_client");
            })
            .await;

        let error = auth_for(&server)
            .bearer()
            .await
            .expect_err("a 401 must not yield a token");

        let message = error.to_string();
        assert!(message.contains("401"), "the status is worth reporting");
        assert!(
            !message.contains("s3cret"),
            "the secret must not appear in an error: {message}"
        );
    }

    #[tokio::test]
    async fn a_static_token_is_returned_unchanged() {
        let auth = ControlPlaneAuth::Static("fixed".to_string());

        assert_eq!(auth.bearer().await.expect("a token"), "fixed");
    }

    /// A trailing slash on the issuer is the kind of configuration difference
    /// that produces a double slash and a 404 nobody attributes to it.
    #[test]
    fn a_trailing_slash_on_the_issuer_is_absorbed() {
        let auth = ControlPlaneAuth::client_credentials(
            "https://id.example.test/realms/aether/",
            "herald-service",
            "s3cret",
        );

        let ControlPlaneAuth::ClientCredentials { token_url, .. } = auth else {
            panic!("expected client credentials");
        };
        assert_eq!(
            token_url,
            "https://id.example.test/realms/aether/protocol/openid-connect/token"
        );
    }
}
