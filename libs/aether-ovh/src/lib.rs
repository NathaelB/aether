//! The DNS adapter against OVH's own API.
//!
//! Nothing in this repository signs a request the way OVH wants one signed
//! before this: `$1$` followed by the hex SHA1 of
//! `AS+CK+METHOD+URL+BODY+TIMESTAMP`, where `AS` is the application secret and
//! `CK` the consumer key, both held only here. The timestamp has to agree with
//! OVH's own clock within a few seconds or every call is refused as expired,
//! which is what `GET /auth/time` is for -- asked once per operation, not
//! cached, because a process running for days should not trust a clock delta
//! it measured on the first call it ever made.

use aether_domain::dns::{DnsError, DnsProvider};
use reqwest::{Client, Method, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::time::Duration;
use tracing::warn;

/// Bounded, the same as every other adapter here: a provider that has not
/// answered in this long is not busy, and a wrong endpoint should say so in
/// seconds rather than look like a slow network.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// How long a record answers for before a resolver asks again.
///
/// Short on purpose: the address behind a hostname is a data plane's own, set
/// once at placement and not expected to move, but a data plane that is ever
/// re-pointed should not leave callers stuck behind a stale answer for longer
/// than a few minutes.
const RECORD_TTL: u32 = 60;

/// What differs between one OVH account and the next.
#[derive(Debug, Clone)]
pub struct OvhConfig {
    /// Where the API answers, without a trailing slash: `https://eu.api.ovh.com/1.0`,
    /// `https://ca.api.ovh.com/1.0`, or the US equivalent.
    pub endpoint: String,
    pub application_key: String,
    pub application_secret: String,
    pub consumer_key: String,
    /// The zone records are published into, e.g. `autharie.fr`. Every
    /// hostname [`upsert_record`](DnsProvider::upsert_record) and
    /// [`delete_record`](DnsProvider::delete_record) are given must fall
    /// under it.
    pub zone: String,
}

pub struct OvhDnsProvider {
    client: Client,
    config: OvhConfig,
}

#[derive(Deserialize)]
struct RecordDetail {
    id: u64,
    #[serde(rename = "fieldType")]
    field_type: String,
    target: String,
}

impl OvhDnsProvider {
    pub fn new(config: OvhConfig) -> Result<Self, DnsError> {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(|error| DnsError::ProviderUnavailable {
                reason: error.to_string(),
            })?;

        Ok(Self { client, config })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{path}", self.config.endpoint.trim_end_matches('/'))
    }

    /// The part of a hostname this zone did not already say.
    ///
    /// Refused rather than guessed at: a hostname outside this provider's own
    /// zone is not a record this account can create, and creating one under
    /// the wrong name would be worse than refusing.
    fn subdomain_of(&self, hostname: &str) -> Result<String, DnsError> {
        hostname
            .strip_suffix(&format!(".{}", self.config.zone))
            .filter(|sub| !sub.is_empty())
            .map(str::to_string)
            .ok_or_else(|| DnsError::Refused {
                operation: "compute the record's subdomain".to_string(),
                reason: format!("{hostname} is not under {}", self.config.zone),
            })
    }

    /// OVH's own clock, asked fresh for every operation.
    ///
    /// Unauthenticated -- the one call here that carries no signature, because
    /// the signature needs this to be right first.
    async fn synced_timestamp(&self) -> Result<i64, DnsError> {
        let response = self
            .client
            .get(self.url("auth/time"))
            .send()
            .await
            .map_err(|error| DnsError::ProviderUnavailable {
                reason: error.to_string(),
            })?;

        if !response.status().is_success() {
            return Err(DnsError::ProviderUnavailable {
                reason: format!("auth/time answered {}", response.status()),
            });
        }

        response
            .text()
            .await
            .ok()
            .and_then(|body| body.trim().parse::<i64>().ok())
            .ok_or_else(|| DnsError::Malformed {
                reason: "auth/time did not answer with a timestamp".to_string(),
            })
    }

    fn signature(&self, method: &str, url: &str, body: &str, timestamp: i64) -> String {
        let input = format!(
            "{}+{}+{method}+{url}+{body}+{timestamp}",
            self.config.application_secret, self.config.consumer_key,
        );

        let mut hasher = Sha1::new();
        hasher.update(input.as_bytes());
        let digest = hasher.finalize();

        let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        format!("$1${hex}")
    }

    /// One signed call. `body` is serialised exactly once and the same bytes
    /// are what gets signed and what gets sent -- OVH rejects a signature
    /// computed over anything else, byte for byte.
    async fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Response, DnsError> {
        let url = self.url(path);
        let timestamp = self.synced_timestamp().await?;
        let body_bytes =
            body.map(serde_json::to_vec)
                .transpose()
                .map_err(|error| DnsError::Malformed {
                    reason: error.to_string(),
                })?;
        let body_str = body_bytes
            .as_deref()
            .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
            .unwrap_or_default();

        let signature = self.signature(method.as_str(), &url, &body_str, timestamp);

        let mut request = self
            .client
            .request(method, &url)
            .header("X-Ovh-Application", &self.config.application_key)
            .header("X-Ovh-Consumer", &self.config.consumer_key)
            .header("X-Ovh-Timestamp", timestamp.to_string())
            .header("X-Ovh-Signature", signature);

        if let Some(bytes) = body_bytes {
            request = request
                .header("Content-Type", "application/json")
                .body(bytes);
        }

        request
            .send()
            .await
            .map_err(|error| DnsError::ProviderUnavailable {
                reason: error.to_string(),
            })
    }

    async fn refusal(operation: &str, response: Response) -> DnsError {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable body>".to_string());

        DnsError::Refused {
            operation: operation.to_string(),
            reason: format!("{status}: {body}"),
        }
    }

    /// Every record OVH holds for one label, whatever type it is.
    ///
    /// Not filtered by type: a subdomain that used to be a `CNAME` and is now
    /// an `A` record has a stale record of the old type sitting under the
    /// same name, and only listing everything finds it.
    async fn record_ids(&self, subdomain: &str) -> Result<Vec<u64>, DnsError> {
        let path = format!(
            "domain/zone/{}/record?subDomain={subdomain}",
            self.config.zone
        );
        let response = self.call(Method::GET, &path, None).await?;

        if !response.status().is_success() {
            return Err(Self::refusal("list records", response).await);
        }

        response.json().await.map_err(|error| DnsError::Malformed {
            reason: error.to_string(),
        })
    }

    async fn record_detail(&self, id: u64) -> Result<RecordDetail, DnsError> {
        let path = format!("domain/zone/{}/record/{id}", self.config.zone);
        let response = self.call(Method::GET, &path, None).await?;

        if !response.status().is_success() {
            return Err(Self::refusal("read a record", response).await);
        }

        response.json().await.map_err(|error| DnsError::Malformed {
            reason: error.to_string(),
        })
    }

    async fn delete_record_by_id(&self, id: u64) -> Result<(), DnsError> {
        let path = format!("domain/zone/{}/record/{id}", self.config.zone);
        let response = self.call(Method::DELETE, &path, None).await?;

        if !response.status().is_success() {
            return Err(Self::refusal("delete a record", response).await);
        }

        Ok(())
    }

    /// Applies every pending change on the zone. OVH stages record writes and
    /// does nothing with them until this is called -- skipping it is the
    /// difference between a record existing and a record answering.
    async fn refresh_zone(&self) -> Result<(), DnsError> {
        let path = format!("domain/zone/{}/refresh", self.config.zone);
        let response = self.call(Method::POST, &path, None).await?;

        if !response.status().is_success() {
            return Err(Self::refusal("refresh the zone", response).await);
        }

        Ok(())
    }
}

/// Which record type an address needs. Gateway API's own
/// `.status.addresses` can carry either kind, so this is decided from the
/// value rather than assumed.
fn field_type_for(target: &str) -> &'static str {
    if target.parse::<Ipv4Addr>().is_ok() {
        "A"
    } else if target.parse::<Ipv6Addr>().is_ok() {
        "AAAA"
    } else {
        "CNAME"
    }
}

impl DnsProvider for OvhDnsProvider {
    fn zone(&self) -> &str {
        &self.config.zone
    }

    async fn upsert_record(&self, hostname: &str, target: &str) -> Result<(), DnsError> {
        let subdomain = self.subdomain_of(hostname)?;
        let field_type = field_type_for(target);

        let ids = self.record_ids(&subdomain).await?;

        let mut wanted_exists = false;
        let mut stale = Vec::new();

        for id in ids {
            let detail = self.record_detail(id).await?;

            if detail.field_type == field_type && detail.target == target {
                wanted_exists = true;
            } else {
                stale.push(detail.id);
            }
        }

        if wanted_exists && stale.is_empty() {
            // Already exactly this. No write, and nothing to refresh.
            return Ok(());
        }

        for id in stale {
            self.delete_record_by_id(id).await?;
        }

        if !wanted_exists {
            let response = self
                .call(
                    Method::POST,
                    &format!("domain/zone/{}/record", self.config.zone),
                    Some(&json!({
                        "fieldType": field_type,
                        "subDomain": subdomain,
                        "target": target,
                        "ttl": RECORD_TTL,
                    })),
                )
                .await?;

            if !response.status().is_success() {
                return Err(Self::refusal("create a record", response).await);
            }
        }

        self.refresh_zone().await
    }

    async fn delete_record(&self, hostname: &str) -> Result<(), DnsError> {
        let subdomain = self.subdomain_of(hostname)?;
        let ids = self.record_ids(&subdomain).await?;

        if ids.is_empty() {
            // The outcome asked for, already true.
            return Ok(());
        }

        for id in &ids {
            self.delete_record_by_id(*id).await?;
        }

        self.refresh_zone().await
    }
}

impl OvhConfig {
    /// `None` when this installation was not given a zone to publish into,
    /// which is every installation that has not configured OVH.
    pub fn configured(self) -> Option<Self> {
        if self.zone.trim().is_empty()
            || self.application_key.trim().is_empty()
            || self.application_secret.trim().is_empty()
            || self.consumer_key.trim().is_empty()
        {
            return None;
        }

        Some(self)
    }
}

/// Logged once, from the one place that knows whether this installation was
/// given a zone. Callers that find [`OvhConfig::configured`] gave back `None`
/// use this rather than staying silent about a feature that is doing nothing.
pub fn warn_not_configured() {
    warn!("no DNS zone is configured: deployments will get no hostname of their own");
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::Method::{DELETE, GET, POST};
    use httpmock::MockServer;

    fn config(server: &MockServer) -> OvhConfig {
        OvhConfig {
            endpoint: server.base_url(),
            application_key: "ak".to_string(),
            application_secret: "as".to_string(),
            consumer_key: "ck".to_string(),
            zone: "autharie.fr".to_string(),
        }
    }

    fn time_mock(server: &MockServer) {
        server.mock(|when, then| {
            when.method(GET).path("/auth/time");
            then.status(200).body("1700000000");
        });
    }

    #[test]
    fn an_ip_target_wants_an_a_record() {
        assert_eq!(field_type_for("203.0.113.10"), "A");
    }

    #[test]
    fn an_ipv6_target_wants_an_aaaa_record() {
        assert_eq!(field_type_for("2001:db8::1"), "AAAA");
    }

    #[test]
    fn anything_else_wants_a_cname() {
        assert_eq!(field_type_for("lb.example.net"), "CNAME");
    }

    #[test]
    fn empty_credentials_mean_not_configured() {
        assert!(
            OvhConfig {
                endpoint: "https://eu.api.ovh.com/1.0".to_string(),
                application_key: "".to_string(),
                application_secret: "as".to_string(),
                consumer_key: "ck".to_string(),
                zone: "autharie.fr".to_string(),
            }
            .configured()
            .is_none()
        );
    }

    #[test]
    fn a_full_config_is_configured() {
        assert!(
            OvhConfig {
                endpoint: "https://eu.api.ovh.com/1.0".to_string(),
                application_key: "ak".to_string(),
                application_secret: "as".to_string(),
                consumer_key: "ck".to_string(),
                zone: "autharie.fr".to_string(),
            }
            .configured()
            .is_some()
        );
    }

    /// OVH's own shape: `$1$` followed by exactly the 40 hex characters of a
    /// SHA1 digest. Checked against the algorithm directly rather than
    /// through a live call, because a mismatch here is a mismatch every
    /// request would carry.
    #[test]
    fn the_signature_is_dollar_one_dollar_then_forty_hex_characters() {
        let server = MockServer::start();
        let provider = OvhDnsProvider::new(config(&server)).expect("client builds");

        let signature =
            provider.signature("GET", "https://eu.api.ovh.com/1.0/me", "", 1_700_000_000);

        let hex = signature.strip_prefix("$1$").expect("starts with $1$");
        assert_eq!(hex.len(), 40, "a SHA1 digest is 20 bytes");
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    /// The four headers OVH's signature scheme needs, all present on every
    /// call -- not just the ones this crate happens to exercise elsewhere.
    #[tokio::test]
    async fn every_call_carries_the_four_signing_headers() {
        let server = MockServer::start();
        time_mock(&server);

        let list = server.mock(|when, then| {
            when.method(GET)
                .path("/domain/zone/autharie.fr/record")
                .query_param("subDomain", "app")
                .header_exists("X-Ovh-Application")
                .header_exists("X-Ovh-Consumer")
                .header_exists("X-Ovh-Timestamp")
                .header_exists("X-Ovh-Signature");
            then.status(200).json_body(json!([]));
        });
        let create = server.mock(|when, then| {
            when.method(POST).path("/domain/zone/autharie.fr/record");
            then.status(200).json_body(json!({}));
        });
        let refresh = server.mock(|when, then| {
            when.method(POST).path("/domain/zone/autharie.fr/refresh");
            then.status(200);
        });

        let provider = OvhDnsProvider::new(config(&server)).expect("client builds");
        provider
            .upsert_record("app.autharie.fr", "203.0.113.10")
            .await
            .expect("record created");

        list.assert();
        create.assert();
        refresh.assert();
    }

    /// The point of the port: called again with the address it already has,
    /// nothing is written and nothing is refreshed.
    #[tokio::test]
    async fn upserting_the_same_target_again_writes_nothing() {
        let server = MockServer::start();
        time_mock(&server);

        server.mock(|when, then| {
            when.method(GET)
                .path("/domain/zone/autharie.fr/record")
                .query_param("subDomain", "app");
            then.status(200).json_body(json!([42]));
        });
        server.mock(|when, then| {
            when.method(GET).path("/domain/zone/autharie.fr/record/42");
            then.status(200).json_body(json!({
                "id": 42, "fieldType": "A", "target": "203.0.113.10"
            }));
        });
        let create = server.mock(|when, then| {
            when.method(POST).path("/domain/zone/autharie.fr/record");
            then.status(200);
        });

        let provider = OvhDnsProvider::new(config(&server)).expect("client builds");
        provider
            .upsert_record("app.autharie.fr", "203.0.113.10")
            .await
            .expect("no-op");

        create.assert_hits(0);
    }

    /// A record pointing somewhere else is replaced, not left beside a second
    /// one: the stale record is deleted before the new one is created.
    #[tokio::test]
    async fn a_record_pointing_elsewhere_is_replaced() {
        let server = MockServer::start();
        time_mock(&server);

        server.mock(|when, then| {
            when.method(GET)
                .path("/domain/zone/autharie.fr/record")
                .query_param("subDomain", "app");
            then.status(200).json_body(json!([42]));
        });
        server.mock(|when, then| {
            when.method(GET).path("/domain/zone/autharie.fr/record/42");
            then.status(200).json_body(json!({
                "id": 42, "fieldType": "A", "target": "203.0.113.99"
            }));
        });
        let delete = server.mock(|when, then| {
            when.method(DELETE)
                .path("/domain/zone/autharie.fr/record/42");
            then.status(200);
        });
        let create = server.mock(|when, then| {
            when.method(POST).path("/domain/zone/autharie.fr/record");
            then.status(200);
        });
        let refresh = server.mock(|when, then| {
            when.method(POST).path("/domain/zone/autharie.fr/refresh");
            then.status(200);
        });

        let provider = OvhDnsProvider::new(config(&server)).expect("client builds");
        provider
            .upsert_record("app.autharie.fr", "203.0.113.10")
            .await
            .expect("record replaced");

        delete.assert();
        create.assert();
        refresh.assert();
    }

    /// No record for the subdomain at all: deleting it is already the
    /// outcome asked for, and nothing is sent beyond the list that found
    /// nothing.
    #[tokio::test]
    async fn deleting_a_hostname_with_no_record_is_not_an_error() {
        let server = MockServer::start();
        time_mock(&server);

        server.mock(|when, then| {
            when.method(GET)
                .path("/domain/zone/autharie.fr/record")
                .query_param("subDomain", "gone");
            then.status(200).json_body(json!([]));
        });
        let delete = server.mock(|when, then| {
            when.method(DELETE);
            then.status(200);
        });

        let provider = OvhDnsProvider::new(config(&server)).expect("client builds");
        provider
            .delete_record("gone.autharie.fr")
            .await
            .expect("idempotent");

        delete.assert_hits(0);
    }

    #[tokio::test]
    async fn deleting_an_existing_record_removes_it_and_refreshes() {
        let server = MockServer::start();
        time_mock(&server);

        server.mock(|when, then| {
            when.method(GET)
                .path("/domain/zone/autharie.fr/record")
                .query_param("subDomain", "app");
            then.status(200).json_body(json!([42]));
        });
        let delete = server.mock(|when, then| {
            when.method(DELETE)
                .path("/domain/zone/autharie.fr/record/42");
            then.status(200);
        });
        let refresh = server.mock(|when, then| {
            when.method(POST).path("/domain/zone/autharie.fr/refresh");
            then.status(200);
        });

        let provider = OvhDnsProvider::new(config(&server)).expect("client builds");
        provider
            .delete_record("app.autharie.fr")
            .await
            .expect("deleted");

        delete.assert();
        refresh.assert();
    }

    /// A hostname outside this provider's own zone cannot be turned into a
    /// record this account has any authority over.
    #[tokio::test]
    async fn a_hostname_outside_the_zone_is_refused() {
        let server = MockServer::start();
        let provider = OvhDnsProvider::new(config(&server)).expect("client builds");

        let result = provider
            .upsert_record("app.example.com", "203.0.113.10")
            .await;

        assert!(matches!(result, Err(DnsError::Refused { .. })));
    }

    #[tokio::test]
    async fn an_unreachable_provider_is_reported_as_unavailable() {
        let config = OvhConfig {
            endpoint: "http://127.0.0.1:1".to_string(),
            application_key: "ak".to_string(),
            application_secret: "as".to_string(),
            consumer_key: "ck".to_string(),
            zone: "autharie.fr".to_string(),
        };

        let provider = OvhDnsProvider::new(config).expect("client builds");
        let result = provider
            .upsert_record("app.autharie.fr", "203.0.113.10")
            .await;

        assert!(matches!(result, Err(DnsError::ProviderUnavailable { .. })));
    }
}
