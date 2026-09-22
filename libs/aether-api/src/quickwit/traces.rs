//! The control plane's own Quickwit trace-search adapter.
//!
//! Mirrors `quickwit::logs`: reads back what
//! `herald-core::infrastructure::traces::quickwit::QuickwitTraceIndexSink`
//! ships into `traces-{organisation_id}`, over the same Quickwit HTTP API,
//! with no dependency on herald-core for the same reason the logs adapter
//! has none.

use std::time::Duration;

use aether_core::{
    CoreError,
    deployments::DeploymentId,
    logs::{LogSearchBucket, LogSearchWindow, histogram_interval},
    organisation::OrganisationId,
    traces::{
        MAX_FACET_TERMS, MAX_SEARCH_HITS, SpanHit, TraceDetail, TraceFacetBucket, TraceFacets,
        TraceSearchFilter, TraceSearchResult, ports::TraceSearchIndex,
    },
};
use chrono::{DateTime, TimeZone, Utc};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use uuid::Uuid;

/// Every span a single `trace()` read may return -- generous, and unlike
/// [`MAX_SEARCH_HITS`] not meant as an ordinary page size: a trace with an
/// unusual span count is still one trace, and this exists only so a
/// pathological one cannot drag an unbounded response through the control
/// plane.
const MAX_TRACE_SPANS: usize = 2_000;

/// How long one search call may take before it is given up on -- matches
/// `quickwit::logs`'s own ceiling.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct QuickwitTraceSearchIndex {
    client: Client,
    base_url: String,
}

impl QuickwitTraceSearchIndex {
    /// `base_url` is Quickwit's own address -- the same instance
    /// `QuickwitLogSearchIndex` talks to, no trailing slash.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .unwrap_or_else(|_| Client::new()),
            base_url: base_url.into(),
        }
    }

    fn search_url(&self, organisation_id: OrganisationId) -> String {
        format!(
            "{}/api/v1/traces-{}/search",
            self.base_url, organisation_id.0
        )
    }

    async fn run_search(
        &self,
        organisation_id: OrganisationId,
        body: serde_json::Value,
    ) -> Result<Option<QuickwitSpanSearchResponse>, CoreError> {
        let response = self
            .client
            .post(self.search_url(organisation_id))
            .json(&body)
            .send()
            .await
            .map_err(|err| {
                CoreError::InternalError(format!("failed to search the trace index: {err}"))
            })?;

        // No index yet means this organisation has never shipped a span: an
        // empty answer, not a fault -- the same "not there yet" the logs
        // adapter reads back from a 404.
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(CoreError::InternalError(format!(
                "quickwit trace search failed with status {status}: {body}"
            )));
        }

        response.json().await.map(Some).map_err(|err| {
            CoreError::InternalError(format!("failed to parse quickwit's response: {err}"))
        })
    }
}

/// A double quote inside free text or a filter value would otherwise close
/// the phrase early -- the same reasoning `quickwit::logs::as_phrase`
/// documents.
fn as_phrase(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "'"))
}

fn build_query(filter: &TraceSearchFilter) -> String {
    let mut clauses = Vec::new();

    if let Some(deployment_id) = filter.deployment_id {
        clauses.push(format!(
            "deployment_id:{}",
            as_phrase(&deployment_id.0.to_string())
        ));
    }

    if let Some(service_name) = filter.service_name.as_ref() {
        clauses.push(format!("service_name:{}", as_phrase(service_name)));
    }

    if let Some(status_code) = filter.status_code.as_ref() {
        clauses.push(format!("status_code:{}", as_phrase(status_code)));
    }

    if let Some(text) = filter.text.as_ref().filter(|text| !text.trim().is_empty()) {
        clauses.push(format!("name:{}", as_phrase(text)));
    }

    if clauses.is_empty() {
        "*".to_string()
    } else {
        clauses.join(" AND ")
    }
}

/// Matches `quickwit::logs::interval_expression` -- every width
/// [`histogram_interval`] produces is a whole number of seconds, minutes or
/// hours.
fn interval_expression(interval: chrono::Duration) -> String {
    let seconds = interval.num_seconds();

    if seconds % 3600 == 0 {
        format!("{}h", seconds / 3600)
    } else if seconds % 60 == 0 {
        format!("{}m", seconds / 60)
    } else {
        format!("{seconds}s")
    }
}

fn aggregations(interval: chrono::Duration) -> serde_json::Value {
    let terms =
        |field: &str| serde_json::json!({ "terms": { "field": field, "size": MAX_FACET_TERMS } });
    serde_json::json!({
        "service_name": terms("service_name"),
        "status_code": terms("status_code"),
        "by_time": {
            "date_histogram": {
                "field": "start_timestamp",
                "fixed_interval": interval_expression(interval),
            }
        }
    })
}

#[derive(Debug, Deserialize)]
struct QuickwitSpanHit {
    trace_id: String,
    span_id: String,
    #[serde(default)]
    parent_span_id: String,
    deployment_id: Uuid,
    service_name: String,
    name: String,
    kind: String,
    start_timestamp: DateTime<Utc>,
    duration_nanos: u64,
    status_code: String,
    #[serde(default)]
    status_message: String,
}

impl From<QuickwitSpanHit> for SpanHit {
    fn from(hit: QuickwitSpanHit) -> Self {
        SpanHit {
            trace_id: hit.trace_id,
            span_id: hit.span_id,
            parent_span_id: hit.parent_span_id,
            deployment_id: DeploymentId(hit.deployment_id),
            service_name: hit.service_name,
            name: hit.name,
            kind: hit.kind,
            start_timestamp: hit.start_timestamp,
            duration_nanos: hit.duration_nanos,
            status_code: hit.status_code,
            status_message: hit.status_message,
        }
    }
}

#[derive(Debug, Deserialize)]
struct QuickwitTermsBucket {
    key: String,
    doc_count: u64,
}

#[derive(Debug, Default, Deserialize)]
struct QuickwitTermsAggregation {
    #[serde(default)]
    buckets: Vec<QuickwitTermsBucket>,
}

impl From<QuickwitTermsAggregation> for Vec<TraceFacetBucket> {
    fn from(aggregation: QuickwitTermsAggregation) -> Self {
        aggregation
            .buckets
            .into_iter()
            .map(|bucket| TraceFacetBucket {
                value: bucket.key,
                count: bucket.doc_count,
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
struct DateHistogramBucket {
    /// Matches `quickwit::logs::DateHistogramBucket::key`'s own comment:
    /// Quickwit encodes this as a JSON float, not an integer.
    key: f64,
    doc_count: u64,
}

#[derive(Debug, Default, Deserialize)]
struct DateHistogramAggregation {
    #[serde(default)]
    buckets: Vec<DateHistogramBucket>,
}

#[derive(Debug, Default, Deserialize)]
struct QuickwitAggregations {
    #[serde(default)]
    service_name: QuickwitTermsAggregation,
    #[serde(default)]
    status_code: QuickwitTermsAggregation,
    #[serde(default)]
    by_time: DateHistogramAggregation,
}

impl From<QuickwitAggregations> for TraceFacets {
    fn from(aggregations: QuickwitAggregations) -> Self {
        TraceFacets {
            service_name: aggregations.service_name.into(),
            status_code: aggregations.status_code.into(),
        }
    }
}

fn histogram_buckets(aggregations: &QuickwitAggregations) -> Vec<LogSearchBucket> {
    let mut buckets: Vec<LogSearchBucket> = aggregations
        .by_time
        .buckets
        .iter()
        .filter_map(|bucket| {
            Utc.timestamp_millis_opt(bucket.key as i64)
                .single()
                .map(|start| LogSearchBucket {
                    start,
                    count: bucket.doc_count,
                })
        })
        .collect();

    buckets.sort_by_key(|bucket| bucket.start);
    buckets
}

#[derive(Debug, Deserialize)]
struct QuickwitSpanSearchResponse {
    num_hits: u64,
    #[serde(default)]
    hits: Vec<QuickwitSpanHit>,
    #[serde(default)]
    aggregations: QuickwitAggregations,
}

/// Quickwit's own sort orders by `start_timestamp` alone -- the same
/// tie-break reasoning as `quickwit::logs::total_order`, on the fields a
/// span actually has.
fn total_order(mut hits: Vec<SpanHit>) -> Vec<SpanHit> {
    hits.sort_by(|a, b| {
        b.start_timestamp
            .cmp(&a.start_timestamp)
            .then_with(|| a.trace_id.cmp(&b.trace_id))
            .then_with(|| a.span_id.cmp(&b.span_id))
    });
    hits
}

impl TraceSearchIndex for QuickwitTraceSearchIndex {
    async fn search(
        &self,
        organisation_id: OrganisationId,
        filter: TraceSearchFilter,
    ) -> Result<TraceSearchResult, CoreError> {
        let body = serde_json::json!({
            "query": build_query(&filter),
            "start_timestamp": filter.window.from.timestamp(),
            "end_timestamp": filter.window.to.timestamp(),
            "max_hits": MAX_SEARCH_HITS,
            "sort_by_field": "-start_timestamp",
            "aggs": aggregations(histogram_interval(&filter.window)),
        });

        let Some(parsed) = self.run_search(organisation_id, body).await? else {
            return Ok(TraceSearchResult {
                hits: vec![],
                total_hits: 0,
                facets: TraceFacets {
                    service_name: vec![],
                    status_code: vec![],
                },
                buckets: vec![],
            });
        };

        let hits = parsed.hits.into_iter().map(SpanHit::from).collect();
        let buckets = histogram_buckets(&parsed.aggregations);

        Ok(TraceSearchResult {
            hits: total_order(hits),
            total_hits: parsed.num_hits,
            facets: parsed.aggregations.into(),
            buckets,
        })
    }

    async fn trace(
        &self,
        organisation_id: OrganisationId,
        trace_id: String,
    ) -> Result<TraceDetail, CoreError> {
        // The widest window this installation's index ever holds -- a
        // trace's own id, not a caller-chosen range, is what scopes this
        // read, so the query reaches every span the retention window could
        // still have regardless of when the search that found it ran. One
        // `now`, not two: calling `Utc::now()` again for `to` would make the
        // span a hair over `MAX_SPAN_DAYS` and the window construction below
        // refuse its own widest case.
        let now = Utc::now();
        let window = LogSearchWindow::new(
            now - chrono::Duration::days(LogSearchWindow::MAX_SPAN_DAYS),
            now,
        )?;

        let body = serde_json::json!({
            "query": format!("trace_id:{}", as_phrase(&trace_id)),
            "start_timestamp": window.from.timestamp(),
            "end_timestamp": window.to.timestamp(),
            "max_hits": MAX_TRACE_SPANS,
            "sort_by_field": "start_timestamp",
        });

        let Some(parsed) = self.run_search(organisation_id, body).await? else {
            return Ok(TraceDetail {
                trace_id,
                spans: vec![],
            });
        };

        let mut spans: Vec<SpanHit> = parsed.hits.into_iter().map(SpanHit::from).collect();
        spans.sort_by_key(|span| span.start_timestamp);

        Ok(TraceDetail { trace_id, spans })
    }
}

/// Lets the domain's generic search method be called with a borrowed
/// adapter -- matches `quickwit::logs`'s own `impl LogSearchIndex for
/// &QuickwitLogSearchIndex`.
impl TraceSearchIndex for &QuickwitTraceSearchIndex {
    async fn search(
        &self,
        organisation_id: OrganisationId,
        filter: TraceSearchFilter,
    ) -> Result<TraceSearchResult, CoreError> {
        (*self).search(organisation_id, filter).await
    }

    async fn trace(
        &self,
        organisation_id: OrganisationId,
        trace_id: String,
    ) -> Result<TraceDetail, CoreError> {
        (*self).trace(organisation_id, trace_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::Method::POST;
    use httpmock::MockServer;

    fn window() -> LogSearchWindow {
        LogSearchWindow::new(
            DateTime::parse_from_rfc3339("2026-09-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            DateTime::parse_from_rfc3339("2026-09-02T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        )
        .expect("an ordinary window")
    }

    fn filter() -> TraceSearchFilter {
        TraceSearchFilter {
            deployment_id: None,
            window: window(),
            service_name: None,
            status_code: None,
            text: None,
        }
    }

    #[test]
    fn an_empty_filter_matches_everything() {
        assert_eq!(build_query(&filter()), "*");
    }

    #[test]
    fn a_service_name_filter_reaches_the_query() {
        let mut filter = filter();
        filter.service_name = Some("ferriskey-api".to_string());

        assert!(build_query(&filter).contains("service_name:\"ferriskey-api\""));
    }

    #[test]
    fn a_double_quote_in_free_text_cannot_close_the_phrase_early() {
        let phrase = as_phrase("\" OR status_code:*");

        assert_eq!(phrase.matches('"').count(), 2);
    }

    #[tokio::test]
    async fn a_search_translates_the_filter_and_returns_the_hits() {
        let server = MockServer::start();
        let organisation_id = OrganisationId(Uuid::new_v4());

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path(format!("/api/v1/traces-{}/search", organisation_id.0))
                .json_body_partial(r#"{"max_hits": 200}"#);
            then.status(200).json_body(serde_json::json!({
                "num_hits": 1,
                "hits": [{
                    "trace_id": "11".repeat(16),
                    "span_id": "22".repeat(8),
                    "parent_span_id": "",
                    "deployment_id": "22222222-2222-2222-2222-222222222222",
                    "service_name": "ferriskey-api",
                    "name": "GET /realms/{realm}",
                    "kind": "server",
                    "start_timestamp": "2026-09-01T08:00:00Z",
                    "duration_nanos": 1_500_000,
                    "status_code": "ok",
                    "status_message": "",
                }]
            }));
        });

        let index = QuickwitTraceSearchIndex::new(server.base_url());
        let result = index
            .search(organisation_id, filter())
            .await
            .expect("a successful search");

        mock.assert();
        assert_eq!(result.total_hits, 1);
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].service_name, "ferriskey-api");
    }

    #[tokio::test]
    async fn a_missing_index_is_read_back_as_no_hits() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST);
            then.status(404).body("index not found");
        });

        let index = QuickwitTraceSearchIndex::new(server.base_url());
        let result = index
            .search(OrganisationId(Uuid::new_v4()), filter())
            .await
            .expect("no index yet is not an error");

        assert_eq!(result.total_hits, 0);
    }

    #[tokio::test]
    async fn a_real_failure_is_not_swallowed() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST);
            then.status(500).body("internal error");
        });

        let index = QuickwitTraceSearchIndex::new(server.base_url());
        let error = index
            .search(OrganisationId(Uuid::new_v4()), filter())
            .await
            .expect_err("a real failure must surface");

        assert!(matches!(error, CoreError::InternalError(_)));
    }

    #[tokio::test]
    async fn a_trace_read_queries_by_trace_id_and_sorts_by_start_time() {
        let server = MockServer::start();
        let organisation_id = OrganisationId(Uuid::new_v4());
        let trace_id = "11".repeat(16);

        server.mock(|when, then| {
            when.method(POST).body_contains(&trace_id);
            then.status(200).json_body(serde_json::json!({
                "num_hits": 2,
                "hits": [
                    {
                        "trace_id": trace_id,
                        "span_id": "22".repeat(8),
                        "parent_span_id": "33".repeat(8),
                        "deployment_id": "22222222-2222-2222-2222-222222222222",
                        "service_name": "ferriskey-api",
                        "name": "child",
                        "kind": "internal",
                        "start_timestamp": "2026-09-01T08:00:01Z",
                        "duration_nanos": 100,
                        "status_code": "ok",
                        "status_message": "",
                    },
                    {
                        "trace_id": trace_id,
                        "span_id": "33".repeat(8),
                        "parent_span_id": "",
                        "deployment_id": "22222222-2222-2222-2222-222222222222",
                        "service_name": "ferriskey-api",
                        "name": "root",
                        "kind": "server",
                        "start_timestamp": "2026-09-01T08:00:00Z",
                        "duration_nanos": 500,
                        "status_code": "ok",
                        "status_message": "",
                    },
                ]
            }));
        });

        let index = QuickwitTraceSearchIndex::new(server.base_url());
        let detail = index
            .trace(organisation_id, trace_id.clone())
            .await
            .expect("a successful read");

        assert_eq!(detail.trace_id, trace_id);
        assert_eq!(detail.spans.len(), 2);
        assert_eq!(detail.spans[0].name, "root");
        assert_eq!(detail.spans[1].name, "child");
    }
}
