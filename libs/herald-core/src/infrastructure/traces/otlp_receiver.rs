//! The one inbound door Herald opens: an OTLP/HTTP trace receiver.
//!
//! Every other integration in this crate reads or polls outward -- see
//! `infrastructure/logs/kubernetes.rs`, `infrastructure/usage`. Traces are
//! the exception because they are push-based by design: an OTLP exporter
//! sends when it has something to say, and nothing here can ask it to wait.
//! This is deliberately the smallest possible server for that: one route,
//! protobuf in, an empty protobuf response out, and every failure that is
//! not a decode error handled by returning a status the sender's own OTLP
//! SDK already knows how to retry against.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use prost::Message;
use tracing::{info, warn};

use crate::domain::ports::{DeploymentResolver, TraceIndexSink};
use crate::domain::trace_index::from_export_request;

const OTLP_CONTENT_TYPE: &str = "application/x-protobuf";

#[derive(Clone)]
struct ReceiverState {
    sink: Arc<dyn TraceIndexSink>,
    resolver: Arc<dyn DeploymentResolver>,
}

/// Listens on `listen_addr` for OTLP/HTTP trace exports until the process
/// stops or binding fails.
///
/// No graceful-shutdown hook: this runs for the lifetime of the Herald
/// process the same way the AMQP connections it shares a runtime with do,
/// and stops the same way they do, by the process exiting.
pub async fn run_otlp_receiver(
    listen_addr: SocketAddr,
    sink: Arc<dyn TraceIndexSink>,
    resolver: Arc<dyn DeploymentResolver>,
) -> std::io::Result<()> {
    let state = ReceiverState { sink, resolver };

    let app = Router::new()
        .route("/v1/traces", post(export_traces))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(listen_addr).await?;
    info!(%listen_addr, "otlp trace receiver listening");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
}

async fn export_traces(
    State(state): State<ReceiverState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    body: axum::body::Bytes,
) -> Response {
    let request = match ExportTraceServiceRequest::decode(body.as_ref()) {
        Ok(request) => request,
        Err(error) => {
            warn!(%error, %peer, "could not decode an inbound OTLP export");
            return protobuf_response(
                StatusCode::BAD_REQUEST,
                &ExportTraceServiceResponse::default(),
            );
        }
    };

    // Unattributable is not the sender's fault -- it may simply be a
    // deployment this shard has not synced yet -- so it gets a status an
    // OTLP SDK already retries with backoff, rather than a silent drop that
    // looks like success.
    let Some((organisation_id, deployment_id)) = state.resolver.resolve(peer.ip()).await else {
        warn!(%peer, "could not attribute this span batch to a known deployment; asking the sender to retry");
        return protobuf_response(
            StatusCode::SERVICE_UNAVAILABLE,
            &ExportTraceServiceResponse::default(),
        );
    };

    let documents = from_export_request(&request, &organisation_id, &deployment_id);
    if documents.is_empty() {
        return protobuf_response(StatusCode::OK, &ExportTraceServiceResponse::default());
    }

    if let Err(error) = state.sink.ship(organisation_id, documents).await {
        warn!(%error, %deployment_id, "failed to ship a batch of spans to the trace index");
        return protobuf_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &ExportTraceServiceResponse::default(),
        );
    }

    protobuf_response(StatusCode::OK, &ExportTraceServiceResponse::default())
}

fn protobuf_response(status: StatusCode, message: &ExportTraceServiceResponse) -> Response {
    let mut response = (status, message.encode_to_vec()).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(OTLP_CONTENT_TYPE),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::deployment::DeploymentId;
    use crate::domain::entities::logs::OrganisationId;
    use crate::domain::error::HeraldError;
    use crate::domain::ports::{MockDeploymentResolver, MockTraceIndexSink};
    use opentelemetry_proto::tonic::resource::v1::Resource;
    use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span};
    use std::sync::Mutex as StdMutex;

    fn sample_request() -> ExportTraceServiceRequest {
        ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: Some(Resource {
                    attributes: vec![],
                    dropped_attributes_count: 0,
                    entity_refs: vec![],
                }),
                scope_spans: vec![ScopeSpans {
                    scope: None,
                    spans: vec![Span {
                        trace_id: vec![0x11; 16],
                        span_id: vec![0x22; 8],
                        name: "GET /".to_string(),
                        start_time_unix_nano: 1_000_000_000,
                        end_time_unix_nano: 1_200_000_000,
                        ..Default::default()
                    }],
                    schema_url: String::new(),
                }],
                schema_url: String::new(),
            }],
        }
    }

    /// A real server on an ephemeral port, wired to whatever mocks the test
    /// supplies -- exercising the actual HTTP wiring (`export_traces`
    /// through `axum::serve`) rather than only the pieces it calls, which
    /// `trace_index` and `quickwit`'s own tests already cover in isolation.
    async fn start_server(
        resolver: MockDeploymentResolver,
        sink: MockTraceIndexSink,
    ) -> (SocketAddr, tokio::task::JoinHandle<std::io::Result<()>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = ReceiverState {
            sink: Arc::new(sink),
            resolver: Arc::new(resolver),
        };
        let app = Router::new()
            .route("/v1/traces", post(export_traces))
            .with_state(state);

        let handle = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
        });

        (addr, handle)
    }

    async fn send_export(addr: SocketAddr, body: Vec<u8>) -> reqwest::Response {
        reqwest::Client::new()
            .post(format!("http://{addr}/v1/traces"))
            .header("content-type", OTLP_CONTENT_TYPE)
            .body(body)
            .send()
            .await
            .expect("the request reaches the server")
    }

    #[tokio::test]
    async fn a_well_formed_export_is_shipped_and_acknowledged() {
        let mut resolver = MockDeploymentResolver::new();
        resolver.expect_resolve().returning(|_| {
            Box::pin(async { Some((OrganisationId::new("org-1"), DeploymentId::new("dep-1"))) })
        });

        let shipped = Arc::new(StdMutex::new(None));
        let shipped_for_closure = Arc::clone(&shipped);
        let mut sink = MockTraceIndexSink::new();
        sink.expect_ship()
            .returning(move |organisation_id, documents| {
                *shipped_for_closure.lock().unwrap() = Some((organisation_id, documents));
                Box::pin(async { Ok(()) })
            });

        let (addr, _handle) = start_server(resolver, sink).await;

        let response = send_export(addr, sample_request().encode_to_vec()).await;

        assert_eq!(response.status(), StatusCode::OK);
        let (organisation_id, documents) =
            shipped.lock().unwrap().take().expect("a batch was shipped");
        assert_eq!(organisation_id, OrganisationId::new("org-1"));
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].name, "GET /");
    }

    /// Unattributable is not the sender's fault -- see `export_traces`'s own
    /// comment -- so it must come back as a status an OTLP SDK retries, not
    /// a silent 200 that reads as delivered.
    #[tokio::test]
    async fn an_unattributable_source_is_told_to_retry_rather_than_dropped_silently() {
        let mut resolver = MockDeploymentResolver::new();
        resolver
            .expect_resolve()
            .returning(|_| Box::pin(async { None }));
        let mut sink = MockTraceIndexSink::new();
        sink.expect_ship().times(0);

        let (addr, _handle) = start_server(resolver, sink).await;

        let response = send_export(addr, sample_request().encode_to_vec()).await;

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn malformed_protobuf_is_refused_without_ever_asking_who_sent_it() {
        let mut resolver = MockDeploymentResolver::new();
        resolver.expect_resolve().times(0);
        let sink = MockTraceIndexSink::new();

        let (addr, _handle) = start_server(resolver, sink).await;

        let response = send_export(addr, vec![0xff, 0xff, 0xff]).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn a_ship_failure_is_reported_as_a_retryable_status() {
        let mut resolver = MockDeploymentResolver::new();
        resolver.expect_resolve().returning(|_| {
            Box::pin(async { Some((OrganisationId::new("org-1"), DeploymentId::new("dep-1"))) })
        });
        let mut sink = MockTraceIndexSink::new();
        sink.expect_ship().returning(|_, _| {
            Box::pin(async {
                Err(HeraldError::Internal {
                    message: "boom".to_string(),
                })
            })
        });

        let (addr, _handle) = start_server(resolver, sink).await;

        let response = send_export(addr, sample_request().encode_to_vec()).await;

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
