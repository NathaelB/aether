//! Shipping already-derived span documents to Quickwit.
//!
//! Mirrors `infrastructure/logs/quickwit.rs` exactly -- same lazy
//! create-on-404 index provisioning, same ndjson ingest, same "no
//! `commit=force`" reasoning -- against `traces-{organisation_id}` instead
//! of `logs-{organisation_id}`.

use std::time::Duration;

use reqwest::{Client, StatusCode};

use crate::domain::entities::logs::OrganisationId;
use crate::domain::error::HeraldError;
use crate::domain::ports::TraceIndexSink;
use crate::domain::trace_index::SpanDocument;

/// The doc mapping frozen alongside the logs one, embedded at compile time
/// for the same reason: rendering it for one organisation is a string
/// substitution over the committed template, not a read.
const INDEX_CONFIG_TEMPLATE: &str =
    include_str!("../../../../../docker/quickwit/trace-index-config.template.yaml");

/// How long one ingest call, or one index-create call, may take before this
/// batch is given up on. Matches `infrastructure/logs/quickwit.rs`.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Ships span documents to `traces-{organisation_id}` over Quickwit's HTTP
/// API, creating the index from the frozen template the first time an
/// organisation ships a span.
pub struct QuickwitTraceIndexSink {
    client: Client,
    base_url: String,
}

impl QuickwitTraceIndexSink {
    /// `base_url` is Quickwit's own address, e.g. `http://quickwit:7280` --
    /// the same instance the logs sink talks to, no trailing slash.
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
        format!("traces-{organisation_id}")
    }

    fn ingest_url(&self, organisation_id: &OrganisationId) -> String {
        format!(
            "{}/api/v1/{}/ingest",
            self.base_url,
            Self::index_id(organisation_id)
        )
    }

    fn indexes_url(&self) -> String {
        format!("{}/api/v1/indexes", self.base_url)
    }

    /// Creates the organisation's trace index from the frozen template. A
    /// concurrent create for the same organisation races the same way the
    /// logs sink's does -- the loser's 400 is the ordinary outcome, not a
    /// failure.
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
                message: format!("failed to create the trace index: {err}"),
            })?;

        if response.status().is_success() || response.status() == StatusCode::BAD_REQUEST {
            return Ok(());
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(HeraldError::Internal {
            message: format!("failed to create the trace index: {status}: {body}"),
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
                message: format!("failed to ship spans: {err}"),
            })
    }
}

impl TraceIndexSink for QuickwitTraceIndexSink {
    fn ship<'a>(
        &'a self,
        organisation_id: OrganisationId,
        documents: Vec<SpanDocument>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), HeraldError>> + Send + 'a>>
    {
        Box::pin(async move {
            let body = ndjson(&documents)?;

            let response = self.ingest(&organisation_id, &body).await?;

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

/// One document per span, newline-delimited -- the body Quickwit's ingest
/// endpoint expects.
fn ndjson(documents: &[SpanDocument]) -> Result<String, HeraldError> {
    documents
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()
        .map(|lines| lines.join("\n"))
        .map_err(|err| HeraldError::Internal {
            message: format!("failed to encode a span for the index: {err}"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::deployment::DeploymentId;
    use chrono::{DateTime, Utc};
    use httpmock::Method::POST;
    use httpmock::MockServer;
    use serde_json::json;

    fn document(name: &str) -> SpanDocument {
        SpanDocument {
            trace_id: "11".repeat(16),
            span_id: "22".repeat(8),
            parent_span_id: String::new(),
            organisation_id: OrganisationId::new("11111111-1111-1111-1111-111111111111"),
            deployment_id: DeploymentId::new("22222222-2222-2222-2222-222222222222"),
            service_name: "ferriskey-api".to_string(),
            name: name.to_string(),
            kind: "server".to_string(),
            start_timestamp: DateTime::parse_from_rfc3339("2026-09-20T08:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            duration_nanos: 1_500_000,
            status_code: "ok".to_string(),
            status_message: String::new(),
            attributes: json!({}),
        }
    }

    #[tokio::test]
    async fn a_batch_is_ingested_as_ndjson_against_the_organisations_trace_index() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/api/v1/traces-11111111-1111-1111-1111-111111111111/ingest")
                .header("content-type", "application/x-ndjson")
                .body_contains("\"name\":\"one\"")
                .body_contains("\"name\":\"two\"");
            then.status(200).json_body(json!({}));
        });

        let sink = QuickwitTraceIndexSink::new(server.base_url());
        sink.ship(
            OrganisationId::new("11111111-1111-1111-1111-111111111111"),
            vec![document("one"), document("two")],
        )
        .await
        .expect("ingest succeeds");

        mock.assert();
    }

    #[tokio::test]
    async fn a_missing_index_is_created_and_the_ingest_retried_once() {
        let server = MockServer::start();

        let ingest = server.mock(|when, then| {
            when.method(POST)
                .path("/api/v1/traces-11111111-1111-1111-1111-111111111111/ingest");
            then.status(404).body("index not found");
        });
        let create = server.mock(|when, then| {
            when.method(POST)
                .path("/api/v1/indexes")
                .header("content-type", "application/yaml")
                .body_contains("index_id: traces-11111111-1111-1111-1111-111111111111");
            then.status(200);
        });

        let sink = QuickwitTraceIndexSink::new(server.base_url());
        let result = sink
            .ship(
                OrganisationId::new("11111111-1111-1111-1111-111111111111"),
                vec![document("one")],
            )
            .await;

        create.assert();
        ingest.assert_hits(2);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn create_index_treats_already_exists_as_success() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/api/v1/indexes");
            then.status(400).body("index already exists");
        });

        let sink = QuickwitTraceIndexSink::new(server.base_url());

        sink.create_index(&OrganisationId::new("11111111-1111-1111-1111-111111111111"))
            .await
            .expect("a 400 for an existing index is not an error");
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
                .path("/api/v1/traces-11111111-1111-1111-1111-111111111111/ingest");
            then.status(500).body("internal error");
        });

        let sink = QuickwitTraceIndexSink::new(server.base_url());
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
