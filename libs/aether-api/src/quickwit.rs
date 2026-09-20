//! The control plane's own Quickwit search adapter.
//!
//! Deliberately not a reuse of `herald-core::infrastructure::logs::quickwit`:
//! that adapter ships lines into the index from the data plane, this one
//! reads them back from the control plane, and the control plane must not
//! depend on herald-core -- a data-plane crate -- just to get one adapter's
//! shape. The two happen to speak to the same Quickwit, over the same HTTP
//! API, and nothing beyond that is shared.

use std::collections::HashSet;
use std::time::Duration;

use aether_core::{
    CoreError,
    deployments::DeploymentId,
    logs::{
        LogFacetBucket, LogFacets, LogLevel, LogSearchBucket, LogSearchFilter, LogSearchHit,
        LogSearchResult, LogSignatureCount, MAX_FACET_TERMS, MAX_SEARCH_HITS, MAX_SIGNATURES,
        histogram_interval, ports::LogSearchIndex,
    },
    organisation::OrganisationId,
};
use chrono::{DateTime, TimeZone, Utc};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use uuid::Uuid;

/// How long one search call may take before it is given up on -- the same
/// ceiling `QuickwitLogIndexSink` in herald-core gives one ingest call.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct QuickwitLogSearchIndex {
    client: Client,
    base_url: String,
}

impl QuickwitLogSearchIndex {
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

    fn search_url(&self, organisation_id: OrganisationId) -> String {
        format!("{}/api/v1/logs-{}/search", self.base_url, organisation_id.0)
    }

    /// The terms aggregation both [`LogSearchIndex::group`] and
    /// [`LogSearchIndex::distinct_fingerprints`] are built from -- `group`
    /// adds a representative message per bucket, `distinct_fingerprints`
    /// only needs the keys, so both start here rather than duplicating the
    /// request.
    async fn terms_by_fingerprint(
        &self,
        organisation_id: OrganisationId,
        filter: &LogSearchFilter,
    ) -> Result<Vec<QuickwitTermsBucket>, CoreError> {
        let body = serde_json::json!({
            "query": build_query(filter),
            "start_timestamp": filter.window.from.timestamp(),
            "end_timestamp": filter.window.to.timestamp(),
            "max_hits": 0,
            "aggs": {
                "by_fingerprint": {
                    "terms": { "field": "fingerprint", "size": MAX_SIGNATURES }
                }
            }
        });

        let response = self
            .client
            .post(self.search_url(organisation_id))
            .json(&body)
            .send()
            .await
            .map_err(|err| {
                CoreError::InternalError(format!("failed to group the log index: {err}"))
            })?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(vec![]);
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(CoreError::InternalError(format!(
                "quickwit aggregation failed with status {status}: {body}"
            )));
        }

        let parsed: QuickwitAggregationResponse = response.json().await.map_err(|err| {
            CoreError::InternalError(format!("failed to parse quickwit's aggregation: {err}"))
        })?;

        Ok(parsed
            .aggregations
            .map(|aggregations| aggregations.by_fingerprint.buckets)
            .unwrap_or_default())
    }

    /// The most recent line carrying one signature -- fetched with its own
    /// query rather than a `top_hits` sub-aggregation, so grouping does not
    /// depend on a Quickwit aggregation this deployment's version may not
    /// carry.
    async fn sample_message(
        &self,
        organisation_id: OrganisationId,
        filter: &LogSearchFilter,
        fingerprint: &str,
    ) -> Result<String, CoreError> {
        let query = format!(
            "{} AND fingerprint:{}",
            build_query(filter),
            as_phrase(fingerprint)
        );
        let body = serde_json::json!({
            "query": query,
            "start_timestamp": filter.window.from.timestamp(),
            "end_timestamp": filter.window.to.timestamp(),
            "max_hits": 1,
            "sort_by_field": "-timestamp",
        });

        let response = self
            .client
            .post(self.search_url(organisation_id))
            .json(&body)
            .send()
            .await
            .map_err(|err| {
                CoreError::InternalError(format!("failed to fetch a signature's sample: {err}"))
            })?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(String::new());
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(CoreError::InternalError(format!(
                "quickwit search failed with status {status}: {body}"
            )));
        }

        let parsed: QuickwitSearchResponse = response.json().await.map_err(|err| {
            CoreError::InternalError(format!("failed to parse quickwit's response: {err}"))
        })?;

        Ok(parsed
            .hits
            .into_iter()
            .next()
            .map(|hit| hit.message)
            .unwrap_or_default())
    }
}

/// A double quote inside free text would otherwise close the phrase early
/// and let whatever follows be parsed as Quickwit's own query syntax -- the
/// one thing a caller must not be able to send. Quoting the whole phrase and
/// dropping the one character that could break out of it is cheaper than a
/// real escape sequence and loses nothing a log line's own text needs.
fn as_phrase(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "'"))
}

/// Every level [`LogLevel::and_above`] names, ORed together -- `unknown`
/// included unconditionally, per #292's decision that a floor never excludes
/// it.
fn level_clause(floor: LogLevel) -> String {
    let alternatives = floor
        .and_above()
        .into_iter()
        .map(|level| format!("level:{level}"))
        .collect::<Vec<_>>()
        .join(" OR ");
    format!("({alternatives})")
}

/// Turns the domain's own filter into the one thing Quickwit's query
/// language understands, so nothing the caller sent ever reaches Quickwit
/// except through this translation.
fn build_query(filter: &LogSearchFilter) -> String {
    let mut clauses = vec![level_clause(filter.level_floor)];

    if let Some(deployment_id) = filter.deployment_id {
        clauses.push(format!(
            "deployment_id:{}",
            as_phrase(&deployment_id.0.to_string())
        ));
    }

    if let Some(text) = filter.text.as_ref().filter(|text| !text.trim().is_empty()) {
        clauses.push(format!("message:{}", as_phrase(text)));
    }

    clauses.join(" AND ")
}

/// A [`histogram_interval`] width, in Quickwit's own `fixed_interval` syntax
/// (e.g. `"30s"`, `"5m"`, `"4h"`) -- every width [`histogram_interval`] can
/// produce is a whole number of seconds, minutes or hours, so this never
/// needs to fall back to anything finer.
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

/// One `terms` aggregation per facet field plus a `date_histogram` at
/// `interval`'s width, all in the same request as the hits -- so the facet
/// counts and the histogram are guaranteed consistent with the hits they sit
/// beside rather than a second or third round trip that could race a write
/// between them.
fn aggregations(interval: chrono::Duration) -> serde_json::Value {
    let terms =
        |field: &str| serde_json::json!({ "terms": { "field": field, "size": MAX_FACET_TERMS } });
    serde_json::json!({
        "level": terms("level"),
        "source": terms("source"),
        "deployment_id": terms("deployment_id"),
        "by_time": {
            "date_histogram": {
                "field": "timestamp",
                "fixed_interval": interval_expression(interval),
            }
        }
    })
}

#[derive(Debug, Deserialize)]
struct QuickwitHit {
    timestamp: DateTime<Utc>,
    deployment_id: Uuid,
    source: String,
    level: String,
    message: String,
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

#[derive(Debug, Deserialize)]
struct DateHistogramBucket {
    /// Milliseconds since the epoch -- Quickwit's own key for a
    /// `date_histogram` bucket, the same convention Elasticsearch uses.
    /// Quickwit encodes it as a JSON float (e.g. `1789891200000.0`), not an
    /// integer, so this has to be `f64` or every bucket fails to parse.
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
    level: QuickwitTermsAggregation,
    #[serde(default)]
    source: QuickwitTermsAggregation,
    #[serde(default)]
    deployment_id: QuickwitTermsAggregation,
    #[serde(default)]
    by_fingerprint: QuickwitTermsAggregation,
    #[serde(default)]
    by_time: DateHistogramAggregation,
}

impl From<QuickwitTermsAggregation> for Vec<LogFacetBucket> {
    fn from(aggregation: QuickwitTermsAggregation) -> Self {
        aggregation
            .buckets
            .into_iter()
            .map(|bucket| LogFacetBucket {
                value: bucket.key,
                count: bucket.doc_count,
            })
            .collect()
    }
}

impl From<QuickwitAggregations> for LogFacets {
    fn from(aggregations: QuickwitAggregations) -> Self {
        LogFacets {
            level: aggregations.level.into(),
            source: aggregations.source.into(),
            deployment_id: aggregations.deployment_id.into(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct QuickwitSearchResponse {
    num_hits: u64,
    #[serde(default)]
    hits: Vec<QuickwitHit>,
    #[serde(default)]
    aggregations: QuickwitAggregations,
}

#[derive(Debug, Deserialize)]
struct QuickwitAggregationResponse {
    #[serde(default)]
    aggregations: Option<QuickwitAggregations>,
}

/// Quickwit's own sort orders by `timestamp` alone; two lines recorded in the
/// same second are otherwise in whatever order the shard that held them
/// happened to return them in, which is not guaranteed to repeat. Sorting
/// again here, on the full tuple, is what makes the same query return the
/// hits in the same order on every call rather than merely the same set of
/// hits.
fn total_order(mut hits: Vec<LogSearchHit>) -> Vec<LogSearchHit> {
    hits.sort_by(|a, b| {
        b.timestamp
            .cmp(&a.timestamp)
            .then_with(|| a.deployment_id.0.cmp(&b.deployment_id.0))
            .then_with(|| a.source.cmp(&b.source))
            .then_with(|| a.message.cmp(&b.message))
    });
    hits
}

/// Turns the raw aggregation buckets into the domain's own type, oldest
/// first -- a millisecond key that does not land on a real instant (should
/// Quickwit's numbering ever change) is dropped rather than guessed at.
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

impl LogSearchIndex for QuickwitLogSearchIndex {
    async fn search(
        &self,
        organisation_id: OrganisationId,
        filter: LogSearchFilter,
    ) -> Result<LogSearchResult, CoreError> {
        let body = serde_json::json!({
            "query": build_query(&filter),
            "start_timestamp": filter.window.from.timestamp(),
            "end_timestamp": filter.window.to.timestamp(),
            "max_hits": MAX_SEARCH_HITS,
            "sort_by_field": "-timestamp",
            "aggs": aggregations(histogram_interval(&filter.window)),
        });

        let response = self
            .client
            .post(self.search_url(organisation_id))
            .json(&body)
            .send()
            .await
            .map_err(|err| {
                CoreError::InternalError(format!("failed to search the log index: {err}"))
            })?;

        // No index yet means this organisation has never shipped a line: an
        // empty answer, not a fault -- the same "not there yet" the ingest
        // side reads back from a 404 before creating one.
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(LogSearchResult {
                hits: vec![],
                total_hits: 0,
                facets: LogFacets {
                    level: vec![],
                    source: vec![],
                    deployment_id: vec![],
                },
                buckets: vec![],
            });
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(CoreError::InternalError(format!(
                "quickwit search failed with status {status}: {body}"
            )));
        }

        let parsed: QuickwitSearchResponse = response.json().await.map_err(|err| {
            CoreError::InternalError(format!("failed to parse quickwit's response: {err}"))
        })?;

        let hits = parsed
            .hits
            .into_iter()
            .map(|hit| LogSearchHit {
                timestamp: hit.timestamp,
                deployment_id: DeploymentId(hit.deployment_id),
                source: hit.source,
                level: hit.level,
                message: hit.message,
            })
            .collect();

        let buckets = histogram_buckets(&parsed.aggregations);

        Ok(LogSearchResult {
            hits: total_order(hits),
            total_hits: parsed.num_hits,
            facets: parsed.aggregations.into(),
            buckets,
        })
    }

    async fn group(
        &self,
        organisation_id: OrganisationId,
        filter: LogSearchFilter,
    ) -> Result<Vec<LogSignatureCount>, CoreError> {
        let buckets = self.terms_by_fingerprint(organisation_id, &filter).await?;

        let mut signatures = Vec::with_capacity(buckets.len());
        for bucket in buckets {
            let sample_message = self
                .sample_message(organisation_id, &filter, &bucket.key)
                .await?;
            signatures.push(LogSignatureCount {
                fingerprint: bucket.key,
                count: bucket.doc_count,
                sample_message,
            });
        }

        Ok(signatures)
    }

    async fn distinct_fingerprints(
        &self,
        organisation_id: OrganisationId,
        filter: LogSearchFilter,
    ) -> Result<HashSet<String>, CoreError> {
        let buckets = self.terms_by_fingerprint(organisation_id, &filter).await?;

        Ok(buckets.into_iter().map(|bucket| bucket.key).collect())
    }
}

/// Lets the domain's generic search method be called with a borrowed
/// adapter, the way `AppState` holds it (`Option<Arc<QuickwitLogSearchIndex>>`)
/// without cloning the client on every request.
impl LogSearchIndex for &QuickwitLogSearchIndex {
    async fn search(
        &self,
        organisation_id: OrganisationId,
        filter: LogSearchFilter,
    ) -> Result<LogSearchResult, CoreError> {
        (*self).search(organisation_id, filter).await
    }

    async fn group(
        &self,
        organisation_id: OrganisationId,
        filter: LogSearchFilter,
    ) -> Result<Vec<LogSignatureCount>, CoreError> {
        (*self).group(organisation_id, filter).await
    }

    async fn distinct_fingerprints(
        &self,
        organisation_id: OrganisationId,
        filter: LogSearchFilter,
    ) -> Result<HashSet<String>, CoreError> {
        (*self).distinct_fingerprints(organisation_id, filter).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::logs::LogSearchWindow;
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

    fn filter() -> LogSearchFilter {
        LogSearchFilter {
            deployment_id: None,
            window: window(),
            level_floor: LogLevel::Warn,
            text: None,
        }
    }

    #[test]
    fn a_level_floor_query_always_includes_unknown() {
        let query = level_clause(LogLevel::Error);

        assert!(query.contains("level:error"));
        assert!(query.contains("level:fatal"));
        assert!(query.contains("level:unknown"));
        assert!(!query.contains("level:warn"));
    }

    /// The one property the issue names by name: a caller's free text must
    /// not be able to break out of its own clause and be parsed as Quickwit
    /// query syntax. A double quote in the text would otherwise close the
    /// phrase early and let whatever follows be parsed as a second clause.
    #[test]
    fn a_double_quote_in_free_text_cannot_close_the_phrase_early() {
        let phrase = as_phrase("\" OR level:*");

        // Exactly the opening and closing quote this function adds -- none
        // from the caller's own text, which is what would let it escape.
        assert_eq!(phrase.matches('"').count(), 2);
        assert!(phrase.starts_with('"') && phrase.ends_with('"'));
    }

    #[test]
    fn free_text_reaches_the_query_as_the_message_clause() {
        let mut filter = filter();
        filter.text = Some("pool exhausted".to_string());

        let query = build_query(&filter);

        assert!(query.contains("message:\"pool exhausted\""));
    }

    #[test]
    fn building_the_same_filter_twice_produces_the_same_query() {
        assert_eq!(build_query(&filter()), build_query(&filter()));
    }

    #[test]
    fn an_interval_lands_on_the_coarsest_unit_it_divides_evenly() {
        assert_eq!(interval_expression(chrono::Duration::seconds(10)), "10s");
        assert_eq!(interval_expression(chrono::Duration::seconds(30)), "30s");
        assert_eq!(interval_expression(chrono::Duration::minutes(5)), "5m");
        assert_eq!(interval_expression(chrono::Duration::hours(1)), "1h");
        assert_eq!(interval_expression(chrono::Duration::hours(4)), "4h");
    }

    #[tokio::test]
    async fn a_search_translates_the_filter_and_returns_the_hits() {
        let server = MockServer::start();
        let organisation_id = OrganisationId(Uuid::new_v4());

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path(format!("/api/v1/logs-{}/search", organisation_id.0))
                .json_body_partial(r#"{"max_hits": 200}"#);
            then.status(200).json_body(serde_json::json!({
                "num_hits": 1,
                "hits": [{
                    "timestamp": "2026-09-01T08:00:00Z",
                    "organisation_id": organisation_id.0,
                    "deployment_id": "22222222-2222-2222-2222-222222222222",
                    "source": "ferriskey-api",
                    "level": "warn",
                    "message": "a second line from org1 at 9am",
                }]
            }));
        });

        let index = QuickwitLogSearchIndex::new(server.base_url());
        let result = index
            .search(organisation_id, filter())
            .await
            .expect("a successful search");

        mock.assert();
        assert_eq!(result.total_hits, 1);
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].level, "warn");
        assert!(result.buckets.is_empty());
    }

    /// The same call that fetches hits also asks for the histogram -- one
    /// round trip to Quickwit, not two -- at the interval `filter()`'s
    /// day-long window selects, and reads the buckets back oldest first.
    #[tokio::test]
    async fn a_search_asks_for_a_date_histogram_and_reads_the_buckets_back() {
        let server = MockServer::start();
        let organisation_id = OrganisationId(Uuid::new_v4());

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path(format!("/api/v1/logs-{}/search", organisation_id.0))
                .json_body_partial(
                    r#"{"aggs": {"by_time": {"date_histogram": {"field": "timestamp", "fixed_interval": "15m"}}}}"#,
                );
            // Quickwit's real shape: the key is a JSON float with a decimal
            // point (`1756713600000.0`), not an integer -- a bucket that
            // parsed fine against an integer fixture failed against a live
            // Quickwit until this was caught.
            then.status(200).json_body(serde_json::json!({
                "num_hits": 2,
                "hits": [],
                "aggregations": {
                    "by_time": {
                        "buckets": [
                            {"key": 1756713600000.0_f64, "doc_count": 3},
                            {"key": 1756713000000.0_f64, "doc_count": 1},
                        ]
                    }
                }
            }));
        });

        let index = QuickwitLogSearchIndex::new(server.base_url());
        let result = index
            .search(organisation_id, filter())
            .await
            .expect("a successful search");

        mock.assert();
        assert_eq!(result.buckets.len(), 2);
        assert!(
            result.buckets[0].start < result.buckets[1].start,
            "oldest first"
        );
        assert_eq!(result.buckets[1].count, 3);
    }

    /// Facets come from the aggregations Quickwit answers alongside the
    /// hits, not from counting the (capped) hits themselves.
    #[tokio::test]
    async fn facets_are_parsed_from_the_aggregations_the_index_returns() {
        let server = MockServer::start();
        let organisation_id = OrganisationId(Uuid::new_v4());

        server.mock(|when, then| {
            when.method(POST)
                .path(format!("/api/v1/logs-{}/search", organisation_id.0))
                .json_body_partial(
                    serde_json::json!({ "aggs": aggregations(histogram_interval(&filter().window)) })
                        .to_string(),
                );
            then.status(200).json_body(serde_json::json!({
                "num_hits": 5,
                "hits": [],
                "aggregations": {
                    "level": {
                        "buckets": [
                            { "key": "warn", "doc_count": 3 },
                            { "key": "info", "doc_count": 2 },
                        ]
                    },
                    "source": {
                        "buckets": [
                            { "key": "ferriskey-api", "doc_count": 3 },
                            { "key": "ferriskey-worker", "doc_count": 2 },
                        ]
                    },
                    "deployment_id": { "buckets": [] },
                }
            }));
        });

        let index = QuickwitLogSearchIndex::new(server.base_url());
        let result = index
            .search(organisation_id, filter())
            .await
            .expect("a successful search");

        assert_eq!(
            result.facets.level,
            vec![
                LogFacetBucket {
                    value: "warn".to_string(),
                    count: 3
                },
                LogFacetBucket {
                    value: "info".to_string(),
                    count: 2
                },
            ]
        );
        assert_eq!(result.facets.source.len(), 2);
        assert!(result.facets.deployment_id.is_empty());
    }

    /// An organisation that never shipped a line has no index yet. That is
    /// an empty answer, not a caller-visible error.
    #[tokio::test]
    async fn a_missing_index_is_read_back_as_no_hits() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST);
            then.status(404).body("index not found");
        });

        let index = QuickwitLogSearchIndex::new(server.base_url());
        let result = index
            .search(OrganisationId(Uuid::new_v4()), filter())
            .await
            .expect("no index yet is not an error");

        assert_eq!(result.total_hits, 0);
        assert!(result.hits.is_empty());
        assert!(result.buckets.is_empty());
    }

    #[tokio::test]
    async fn a_real_failure_is_not_swallowed() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST);
            then.status(500).body("internal error");
        });

        let index = QuickwitLogSearchIndex::new(server.base_url());
        let error = index
            .search(OrganisationId(Uuid::new_v4()), filter())
            .await
            .expect_err("a real failure must surface");

        assert!(matches!(error, CoreError::InternalError(_)));
    }

    /// Two hits sharing a timestamp are the case Quickwit's own sort leaves
    /// unspecified; the adapter's own tie-break has to produce the same
    /// order regardless of the order the response listed them in.
    #[tokio::test]
    async fn hits_sharing_a_timestamp_are_still_in_a_fixed_order() {
        let server = MockServer::start();
        let organisation_id = OrganisationId(Uuid::new_v4());

        server.mock(|when, then| {
            when.method(POST);
            then.status(200).json_body(serde_json::json!({
                "num_hits": 2,
                "hits": [
                    {
                        "timestamp": "2026-09-01T08:00:00Z",
                        "organisation_id": organisation_id.0,
                        "deployment_id": "22222222-2222-2222-2222-222222222222",
                        "source": "worker",
                        "level": "error",
                        "message": "zzz second alphabetically last",
                    },
                    {
                        "timestamp": "2026-09-01T08:00:00Z",
                        "organisation_id": organisation_id.0,
                        "deployment_id": "22222222-2222-2222-2222-222222222222",
                        "source": "worker",
                        "level": "error",
                        "message": "aaa alphabetically first",
                    },
                ]
            }));
        });

        let index = QuickwitLogSearchIndex::new(server.base_url());
        let first = index
            .search(organisation_id, filter())
            .await
            .expect("a successful search");
        let second = index
            .search(organisation_id, filter())
            .await
            .expect("a successful search, run again");

        assert_eq!(first.hits, second.hits);
        assert_eq!(first.hits[0].message, "aaa alphabetically first");
        assert_eq!(first.hits[1].message, "zzz second alphabetically last");
    }

    mod grouping {
        use super::*;

        #[tokio::test]
        async fn a_group_returns_one_signature_per_bucket_with_a_sample_message() {
            let server = MockServer::start();
            let organisation_id = OrganisationId(Uuid::new_v4());

            server.mock(|when, then| {
                when.method(POST)
                    .path(format!("/api/v1/logs-{}/search", organisation_id.0))
                    .json_body_partial(r#"{"max_hits": 0}"#);
                then.status(200).json_body(serde_json::json!({
                    "num_hits": 53,
                    "hits": [],
                    "aggregations": {
                        "by_fingerprint": {
                            "buckets": [
                                {"key": "aaaaaaaaaaaaaaaa", "doc_count": 50},
                                {"key": "bbbbbbbbbbbbbbbb", "doc_count": 3},
                            ]
                        }
                    }
                }));
            });
            server.mock(|when, then| {
                when.method(POST)
                    .path(format!("/api/v1/logs-{}/search", organisation_id.0))
                    .json_body_partial(r#"{"max_hits": 1}"#)
                    .body_contains("aaaaaaaaaaaaaaaa");
                then.status(200).json_body(serde_json::json!({
                    "num_hits": 50,
                    "hits": [{
                        "timestamp": "2026-09-01T08:00:00Z",
                        "deployment_id": "22222222-2222-2222-2222-222222222222",
                        "source": "ferriskey-api",
                        "level": "error",
                        "message": "the pool is exhausted",
                    }]
                }));
            });
            server.mock(|when, then| {
                when.method(POST)
                    .path(format!("/api/v1/logs-{}/search", organisation_id.0))
                    .json_body_partial(r#"{"max_hits": 1}"#)
                    .body_contains("bbbbbbbbbbbbbbbb");
                then.status(200).json_body(serde_json::json!({
                    "num_hits": 3,
                    "hits": [{
                        "timestamp": "2026-09-01T08:00:00Z",
                        "deployment_id": "22222222-2222-2222-2222-222222222222",
                        "source": "ferriskey-api",
                        "level": "warn",
                        "message": "a different signature",
                    }]
                }));
            });

            let index = QuickwitLogSearchIndex::new(server.base_url());
            let signatures = index
                .group(organisation_id, filter())
                .await
                .expect("a successful group");

            assert_eq!(signatures.len(), 2);
            let burst = signatures
                .iter()
                .find(|signature| signature.fingerprint == "aaaaaaaaaaaaaaaa")
                .expect("present");
            assert_eq!(burst.count, 50);
            assert_eq!(burst.sample_message, "the pool is exhausted");
        }

        #[tokio::test]
        async fn distinct_fingerprints_reads_back_only_the_keys() {
            let server = MockServer::start();
            let organisation_id = OrganisationId(Uuid::new_v4());

            server.mock(|when, then| {
                when.method(POST);
                then.status(200).json_body(serde_json::json!({
                    "num_hits": 0,
                    "hits": [],
                    "aggregations": {
                        "by_fingerprint": {
                            "buckets": [{"key": "cccccccccccccccc", "doc_count": 7}]
                        }
                    }
                }));
            });

            let index = QuickwitLogSearchIndex::new(server.base_url());
            let fingerprints = index
                .distinct_fingerprints(organisation_id, filter())
                .await
                .expect("a successful aggregation");

            assert_eq!(
                fingerprints,
                HashSet::from(["cccccccccccccccc".to_string()])
            );
        }

        /// No index yet is an empty answer here too, the same as a plain
        /// search -- an organisation that never shipped a line has no
        /// signatures, not a fault.
        #[tokio::test]
        async fn a_missing_index_groups_to_nothing() {
            let server = MockServer::start();
            server.mock(|when, then| {
                when.method(POST);
                then.status(404).body("index not found");
            });

            let index = QuickwitLogSearchIndex::new(server.base_url());
            let signatures = index
                .group(OrganisationId(Uuid::new_v4()), filter())
                .await
                .expect("no index yet is not an error");

            assert!(signatures.is_empty());
        }
    }
}
