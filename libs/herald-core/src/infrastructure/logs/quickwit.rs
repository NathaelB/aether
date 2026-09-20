//! Shipping already-derived log documents to Quickwit.
//!
//! Best-effort by construction: nothing in [`crate::domain::log_session`]
//! ever waits on this beyond the timeout below, and every error this adapter
//! returns is logged and swallowed by its caller, the same way a failed
//! heartbeat or outcome report already is.

use std::time::Duration;

use reqwest::{Client, StatusCode};

use crate::domain::entities::logs::OrganisationId;
use crate::domain::error::HeraldError;
use crate::domain::log_index::LogIndexDocument;
use crate::domain::ports::LogIndexSink;

/// The doc mapping #293 froze, embedded at compile time so this adapter
/// needs no file access at runtime -- rendering it for one organisation is a
/// string substitution over the committed template, not a read.
const INDEX_CONFIG_TEMPLATE: &str =
    include_str!("../../../../../docker/quickwit/index-config.template.yaml");

/// How long one ingest call, or one index-create call, may take before this
/// batch is given up on.
///
/// Bounded for the same reason `LIST_TIMEOUT` is in
/// `infrastructure/logs/kubernetes.rs`: a Quickwit that hangs must not hold a
/// request open forever, even though shipping already runs off the
/// continuous reader's own read loop (see `log_shipping::ship`).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Ships log documents to `logs-{organisation_id}` over Quickwit's HTTP API,
/// creating the index from the frozen template the first time an
/// organisation ships a line.
pub struct QuickwitLogIndexSink {
    client: Client,
    base_url: String,
}

impl QuickwitLogIndexSink {
    /// `base_url` is Quickwit's own address, e.g. `http://quickwit:7280` --
    /// no trailing slash.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .unwrap_or_else(|_| Client::new()),
            base_url: base_url.into(),
        }
    }

    fn index_id(organisation_id: &OrganisationId) -> String {
        format!("logs-{organisation_id}")
    }

    fn ingest_url(&self, organisation_id: &OrganisationId) -> String {
        // No `commit=force`: that was defensible at one live session's
        // volume, but a continuous reader per deployment across a fleet
        // (#294) would publish a split every `FLUSH_INTERVAL` per
        // deployment, which is exactly what Quickwit's own docs warn a
        // forced commit does to small-split overhead. The index's own
        // `commit_timeout_secs` (docker/quickwit/index-config.template.yaml)
        // is the bound on findability now, not this call.
        format!(
            "{}/api/v1/{}/ingest",
            self.base_url,
            Self::index_id(organisation_id)
        )
    }

    fn indexes_url(&self) -> String {
        format!("{}/api/v1/indexes", self.base_url)
    }

    /// Creates the organisation's index from the frozen template.
    ///
    /// A concurrent session for the same organisation can race here -- two
    /// sessions shipping their first-ever batch at once -- and Quickwit
    /// answers the loser with a 400 saying the index already exists. That is
    /// the ordinary outcome of the race, not a failure, so it is read back as
    /// success.
    async fn create_index(&self, organisation_id: &OrganisationId) -> Result<(), HeraldError> {
        let config = INDEX_CONFIG_TEMPLATE.replace("{organisation_id}", &organisation_id.0);

        let response = self
            .client
            .post(self.indexes_url())
            .header("content-type", "application/yaml")
            .body(config)
            .send()
            .await
            .map_err(|err| HeraldError::Internal {
                message: format!("failed to create the log index: {err}"),
            })?;

        if response.status().is_success() || response.status() == StatusCode::BAD_REQUEST {
            return Ok(());
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(HeraldError::Internal {
            message: format!("failed to create the log index: {status}: {body}"),
        })
    }

    async fn ingest(
        &self,
        organisation_id: &OrganisationId,
        body: &str,
    ) -> Result<reqwest::Response, HeraldError> {
        self.client
            .post(self.ingest_url(organisation_id))
            .header("content-type", "application/x-ndjson")
            .body(body.to_string())
            .send()
            .await
            .map_err(|err| HeraldError::Internal {
                message: format!("failed to ship log lines: {err}"),
            })
    }
}

impl LogIndexSink for QuickwitLogIndexSink {
    fn ship<'a>(
        &'a self,
        organisation_id: OrganisationId,
        documents: Vec<LogIndexDocument>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), HeraldError>> + Send + 'a>>
    {
        Box::pin(async move {
            let body = ndjson(&documents)?;

            let response = self.ingest(&organisation_id, &body).await?;

            // The index does not exist yet: created on first write, per
            // #294. Every batch after this one for the same organisation
            // finds it already there.
            let response = if response.status() == StatusCode::NOT_FOUND {
                self.create_index(&organisation_id).await?;
                self.ingest(&organisation_id, &body).await?
            } else {
                response
            };

            ensure_success(response).await
        })
    }
}

async fn ensure_success(response: reqwest::Response) -> Result<(), HeraldError> {
    if response.status().is_success() {
        return Ok(());
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    Err(HeraldError::Internal {
        message: format!("quickwit ingest failed with status {status}: {body}"),
    })
}

/// One document per line, newline-delimited -- the body Quickwit's ingest
/// endpoint expects.
fn ndjson(documents: &[LogIndexDocument]) -> Result<String, HeraldError> {
    documents
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()
        .map(|lines| lines.join("\n"))
        .map_err(|err| HeraldError::Internal {
            message: format!("failed to encode a log line for the index: {err}"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::deployment::DeploymentId;
    use chrono::{DateTime, Utc};
    use httpmock::Method::POST;
    use httpmock::MockServer;

    fn document(message: &str) -> LogIndexDocument {
        LogIndexDocument {
            timestamp: DateTime::parse_from_rfc3339("2026-09-20T08:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            organisation_id: OrganisationId::new("11111111-1111-1111-1111-111111111111"),
            deployment_id: DeploymentId::new("22222222-2222-2222-2222-222222222222"),
            source: "ferriskey-api".to_string(),
            level: "info".to_string(),
            message: message.to_string(),
        }
    }

    #[tokio::test]
    async fn a_batch_is_ingested_as_ndjson_against_the_organisations_index() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/api/v1/logs-11111111-1111-1111-1111-111111111111/ingest")
                .header("content-type", "application/x-ndjson")
                .body_contains("\"message\":\"one\"")
                .body_contains("\"message\":\"two\"");
            then.status(200).json_body(serde_json::json!({}));
        });

        let sink = QuickwitLogIndexSink::new(server.base_url());
        sink.ship(
            OrganisationId::new("11111111-1111-1111-1111-111111111111"),
            vec![document("one"), document("two")],
        )
        .await
        .expect("ingest succeeds");

        mock.assert();
    }

    /// The issue's own words: created on first write if it does not exist
    /// yet. Both mocked ingest attempts answer 404 here -- httpmock has no
    /// notion of "the second call differs from the first" -- so this proves
    /// the orchestration (ingest, create on 404, ingest again) rather than a
    /// successful end state; `a_batch_is_ingested_as_ndjson_against_the_organisations_index`
    /// above already covers a plain successful ingest.
    #[tokio::test]
    async fn a_missing_index_is_created_and_the_ingest_retried_once() {
        let server = MockServer::start();

        let ingest = server.mock(|when, then| {
            when.method(POST)
                .path("/api/v1/logs-11111111-1111-1111-1111-111111111111/ingest");
            then.status(404).body("index not found");
        });
        let create = server.mock(|when, then| {
            when.method(POST)
                .path("/api/v1/indexes")
                .header("content-type", "application/yaml")
                .body_contains("index_id: logs-11111111-1111-1111-1111-111111111111");
            then.status(200);
        });

        let sink = QuickwitLogIndexSink::new(server.base_url());
        let result = sink
            .ship(
                OrganisationId::new("11111111-1111-1111-1111-111111111111"),
                vec![document("one")],
            )
            .await;

        create.assert();
        // Once for the first 404, once for the retry after creation.
        ingest.assert_hits(2);
        match result {
            Err(HeraldError::Internal { message }) => {
                assert!(message.contains("404"), "{message}")
            }
            other => panic!("expected the retried response's own failure, got {other:?}"),
        }
    }

    /// Two sessions shipping the same organisation's first-ever batch race
    /// on creating the index. The loser's 400 is the ordinary outcome of
    /// that race, not a failure worth propagating.
    #[tokio::test]
    async fn create_index_treats_already_exists_as_success() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/api/v1/indexes");
            then.status(400).body("index already exists");
        });

        let sink = QuickwitLogIndexSink::new(server.base_url());

        sink.create_index(&OrganisationId::new("11111111-1111-1111-1111-111111111111"))
            .await
            .expect("a 400 for an existing index is not an error");
    }

    #[tokio::test]
    async fn create_index_propagates_other_failures() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/api/v1/indexes");
            then.status(500).body("internal error");
        });

        let sink = QuickwitLogIndexSink::new(server.base_url());

        let error = sink
            .create_index(&OrganisationId::new("11111111-1111-1111-1111-111111111111"))
            .await
            .expect_err("a real failure must not be swallowed");

        assert!(matches!(error, HeraldError::Internal { .. }));
    }

    #[tokio::test]
    async fn a_non_404_ingest_failure_is_not_retried_as_a_missing_index() {
        let server = MockServer::start();
        let create = server.mock(|when, then| {
            when.method(POST).path("/api/v1/indexes");
            then.status(200);
        });
        server.mock(|when, then| {
            when.method(POST)
                .path("/api/v1/logs-11111111-1111-1111-1111-111111111111/ingest");
            then.status(500).body("internal error");
        });

        let sink = QuickwitLogIndexSink::new(server.base_url());
        let result = sink
            .ship(
                OrganisationId::new("11111111-1111-1111-1111-111111111111"),
                vec![document("one")],
            )
            .await;

        assert!(result.is_err());
        create.assert_hits(0);
    }
}
