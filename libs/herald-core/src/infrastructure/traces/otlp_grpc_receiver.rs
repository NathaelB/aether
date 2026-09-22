//! The gRPC half of Herald's OTLP door.
//!
//! `otlp_receiver` (OTLP/HTTP) was written first on the assumption that any
//! correctly configured OTLP exporter would do -- but FerrisKey's own
//! exporter, confirmed against a live instance, defaults to gRPC rather
//! than HTTP/protobuf. Both are equally valid OTLP transports (the spec's
//! own 4317/4318 port convention exists precisely because installations
//! differ here), so this is the other half of the door, not a replacement
//! for it: same domain logic, same attribution, a different wire protocol.

use std::net::SocketAddr;
use std::sync::Arc;

use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
    trace_service_server::{TraceService, TraceServiceServer},
};
use tonic::{Request, Response, Status, transport::Server};
use tracing::{info, warn};

use crate::domain::ports::{DeploymentResolver, TraceIndexSink};
use crate::domain::trace_index::from_export_request;

struct GrpcTraceReceiver {
    sink: Arc<dyn TraceIndexSink>,
    resolver: Arc<dyn DeploymentResolver>,
}

#[tonic::async_trait]
impl TraceService for GrpcTraceReceiver {
    async fn export(
        &self,
        request: Request<ExportTraceServiceRequest>,
    ) -> Result<Response<ExportTraceServiceResponse>, Status> {
        // Populated by `tonic::transport::Server` from the connection itself,
        // never from anything the client sent -- the same trust boundary
        // `otlp_receiver::export_traces` reads `ConnectInfo` from.
        let peer = request
            .remote_addr()
            .ok_or_else(|| Status::internal("no peer address on this connection"))?;
        let request = request.into_inner();

        // Unattributable is not the sender's fault -- see
        // `otlp_receiver::export_traces`'s own comment -- so it gets a gRPC
        // code an OTLP SDK already retries (`UNAVAILABLE` is on the OTLP
        // spec's retryable list), rather than an error that reads as this
        // batch being rejected for good.
        let Some((organisation_id, deployment_id)) = self.resolver.resolve(peer.ip()).await else {
            warn!(%peer, "could not attribute this span batch to a known deployment; asking the sender to retry");
            return Err(Status::unavailable(
                "could not attribute this span batch to a known deployment",
            ));
        };

        let documents = from_export_request(&request, &organisation_id, &deployment_id);
        if documents.is_empty() {
            return Ok(Response::new(ExportTraceServiceResponse::default()));
        }

        if let Err(error) = self.sink.ship(organisation_id, documents).await {
            warn!(%error, %deployment_id, "failed to ship a batch of spans to the trace index");
            return Err(Status::internal("failed to ship spans to the trace index"));
        }

        Ok(Response::new(ExportTraceServiceResponse::default()))
    }
}

/// Listens on `listen_addr` for OTLP/gRPC trace exports until the process
/// stops or binding fails -- the gRPC counterpart to
/// [`super::otlp_receiver::run_otlp_receiver`].
pub async fn run_otlp_grpc_receiver(
    listen_addr: SocketAddr,
    sink: Arc<dyn TraceIndexSink>,
    resolver: Arc<dyn DeploymentResolver>,
) -> Result<(), tonic::transport::Error> {
    info!(%listen_addr, "otlp grpc trace receiver listening");

    Server::builder()
        .add_service(TraceServiceServer::new(GrpcTraceReceiver {
            sink,
            resolver,
        }))
        .serve(listen_addr)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::deployment::DeploymentId;
    use crate::domain::entities::logs::OrganisationId;
    use crate::domain::error::HeraldError;
    use crate::domain::ports::{MockDeploymentResolver, MockTraceIndexSink};
    use opentelemetry_proto::tonic::collector::trace::v1::trace_service_client::TraceServiceClient;
    use opentelemetry_proto::tonic::resource::v1::Resource;
    use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span};
    use std::sync::Mutex as StdMutex;
    use tonic::Code;

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

    async fn start_server(
        resolver: MockDeploymentResolver,
        sink: MockTraceIndexSink,
    ) -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let receiver = GrpcTraceReceiver {
            sink: Arc::new(sink),
            resolver: Arc::new(resolver),
        };
        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

        let handle = tokio::spawn(async move {
            Server::builder()
                .add_service(TraceServiceServer::new(receiver))
                .serve_with_incoming(incoming)
                .await
                .ok();
        });

        (addr, handle)
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
        let mut client = TraceServiceClient::connect(format!("http://{addr}"))
            .await
            .expect("connects");

        let response = client
            .export(sample_request())
            .await
            .expect("the export succeeds");
        assert!(response.into_inner().partial_success.is_none());

        let (organisation_id, documents) =
            shipped.lock().unwrap().take().expect("a batch was shipped");
        assert_eq!(organisation_id, OrganisationId::new("org-1"));
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].name, "GET /");
    }

    #[tokio::test]
    async fn an_unattributable_source_is_told_to_retry_rather_than_dropped_silently() {
        let mut resolver = MockDeploymentResolver::new();
        resolver
            .expect_resolve()
            .returning(|_| Box::pin(async { None }));
        let mut sink = MockTraceIndexSink::new();
        sink.expect_ship().times(0);

        let (addr, _handle) = start_server(resolver, sink).await;
        let mut client = TraceServiceClient::connect(format!("http://{addr}"))
            .await
            .expect("connects");

        let error = client.export(sample_request()).await.expect_err("refused");
        assert_eq!(error.code(), Code::Unavailable);
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
        let mut client = TraceServiceClient::connect(format!("http://{addr}"))
            .await
            .expect("connects");

        let error = client.export(sample_request()).await.expect_err("refused");
        assert_eq!(error.code(), Code::Internal);
    }
}
