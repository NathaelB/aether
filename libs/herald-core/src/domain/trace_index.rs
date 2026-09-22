//! Turning an OTLP span into the document the trace index expects.
//!
//! Mirrors [`super::log_index`]: the fields below are the mapping frozen at
//! `docker/quickwit/trace-index-config.template.yaml`, and this is the one
//! place that maps a decoded `Span` onto it.

use chrono::{DateTime, Utc};
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::common::v1::{
    AnyValue, KeyValue, any_value::Value as AnyValueKind,
};
use opentelemetry_proto::tonic::trace::v1::{
    ResourceSpans, Span, span::SpanKind, status::StatusCode,
};
use serde::Serialize;
use serde_json::{Map, Value};

use super::entities::deployment::DeploymentId;
use super::entities::logs::OrganisationId;

/// The service name a resource carries no attribute for.
///
/// Named rather than left absent: a facet over `service_name` that silently
/// dropped spans with no resource attributes would undercount a trace, not
/// merely mislabel it.
const UNKNOWN_SERVICE: &str = "unknown_service";

/// One span, shaped for the index.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpanDocument {
    pub trace_id: String,
    pub span_id: String,
    /// Empty for a root span, matching the wire representation rather than
    /// `Option` -- a root span's parent is not unknown, it does not exist.
    pub parent_span_id: String,
    pub organisation_id: OrganisationId,
    pub deployment_id: DeploymentId,
    pub service_name: String,
    pub name: String,
    pub kind: String,
    pub start_timestamp: DateTime<Utc>,
    pub duration_nanos: u64,
    pub status_code: String,
    pub status_message: String,
    pub attributes: Value,
}

impl SpanDocument {
    /// Every span in every scope of one `ResourceSpans` -- one resource, and
    /// therefore one `service_name`, covering all of them.
    pub fn from_resource_spans(
        resource_spans: &ResourceSpans,
        organisation_id: &OrganisationId,
        deployment_id: &DeploymentId,
    ) -> Vec<Self> {
        let service_name = resource_spans
            .resource
            .as_ref()
            .map(|resource| service_name(&resource.attributes))
            .unwrap_or_else(|| UNKNOWN_SERVICE.to_string());

        resource_spans
            .scope_spans
            .iter()
            .flat_map(|scope_spans| &scope_spans.spans)
            .map(|span| Self::from_span(span, organisation_id, deployment_id, &service_name))
            .collect()
    }

    fn from_span(
        span: &Span,
        organisation_id: &OrganisationId,
        deployment_id: &DeploymentId,
        service_name: &str,
    ) -> Self {
        let status = span.status.as_ref();

        Self {
            trace_id: hex_encode(&span.trace_id),
            span_id: hex_encode(&span.span_id),
            parent_span_id: hex_encode(&span.parent_span_id),
            organisation_id: organisation_id.clone(),
            deployment_id: deployment_id.clone(),
            service_name: service_name.to_string(),
            name: span.name.clone(),
            kind: span_kind(span.kind),
            start_timestamp: nanos_to_timestamp(span.start_time_unix_nano),
            duration_nanos: span
                .end_time_unix_nano
                .saturating_sub(span.start_time_unix_nano),
            status_code: status
                .map(|status| status_code(status.code))
                .unwrap_or_else(|| status_code(StatusCode::Unset as i32)),
            status_message: status
                .map(|status| status.message.clone())
                .unwrap_or_default(),
            attributes: attributes_to_json(&span.attributes),
        }
    }
}

/// Every span and event this crate ever ships carries a fixed set of ids and
/// timings; only `attributes` is an open bag, so it is the one field kept as
/// nested JSON rather than flattened into named columns (the index's own
/// `type: json` mapping, see the template).
fn attributes_to_json(attributes: &[KeyValue]) -> Value {
    let mut map = Map::with_capacity(attributes.len());
    for attribute in attributes {
        if let Some(value) = attribute.value.as_ref() {
            map.insert(attribute.key.clone(), any_value_to_json(value));
        }
    }
    Value::Object(map)
}

fn any_value_to_json(value: &AnyValue) -> Value {
    match value.value.as_ref() {
        Some(AnyValueKind::StringValue(value)) => Value::String(value.clone()),
        Some(AnyValueKind::BoolValue(value)) => Value::Bool(*value),
        Some(AnyValueKind::IntValue(value)) => Value::Number((*value).into()),
        Some(AnyValueKind::DoubleValue(value)) => {
            serde_json::Number::from_f64(*value).map_or(Value::Null, Value::Number)
        }
        Some(AnyValueKind::ArrayValue(array)) => {
            Value::Array(array.values.iter().map(any_value_to_json).collect())
        }
        Some(AnyValueKind::KvlistValue(list)) => attributes_to_json(&list.values),
        Some(AnyValueKind::BytesValue(bytes)) => Value::String(hex_encode(bytes)),
        Some(AnyValueKind::StringValueStrindex(_)) | None => Value::Null,
    }
}

/// The resource attribute every OTel SDK sets by convention
/// (`service.name`), read defensively: a resource that omits it is still a
/// resource, not a reason to fail the whole batch.
fn service_name(attributes: &[KeyValue]) -> String {
    attributes
        .iter()
        .find(|attribute| attribute.key == "service.name")
        .and_then(|attribute| attribute.value.as_ref())
        .and_then(|value| match value.value.as_ref() {
            Some(AnyValueKind::StringValue(value)) => Some(value.clone()),
            _ => None,
        })
        .unwrap_or_else(|| UNKNOWN_SERVICE.to_string())
}

fn span_kind(kind: i32) -> String {
    match SpanKind::try_from(kind).unwrap_or(SpanKind::Unspecified) {
        SpanKind::Unspecified => "unspecified",
        SpanKind::Internal => "internal",
        SpanKind::Server => "server",
        SpanKind::Client => "client",
        SpanKind::Producer => "producer",
        SpanKind::Consumer => "consumer",
    }
    .to_string()
}

fn status_code(code: i32) -> String {
    match StatusCode::try_from(code).unwrap_or(StatusCode::Unset) {
        StatusCode::Unset => "unset",
        StatusCode::Ok => "ok",
        StatusCode::Error => "error",
    }
    .to_string()
}

fn nanos_to_timestamp(unix_nanos: u64) -> DateTime<Utc> {
    DateTime::from_timestamp(
        (unix_nanos / 1_000_000_000) as i64,
        (unix_nanos % 1_000_000_000) as u32,
    )
    .unwrap_or_else(Utc::now)
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Every `ResourceSpans` in one OTLP export, attributed to the one
/// deployment the push was resolved to (see
/// `infrastructure/traces/otlp_receiver`) -- an export always comes from one
/// sender, so every resource in it belongs to the same deployment even
/// though each may name a different `service_name`.
pub fn from_export_request(
    request: &ExportTraceServiceRequest,
    organisation_id: &OrganisationId,
    deployment_id: &DeploymentId,
) -> Vec<SpanDocument> {
    request
        .resource_spans
        .iter()
        .flat_map(|resource_spans| {
            SpanDocument::from_resource_spans(resource_spans, organisation_id, deployment_id)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry_proto::tonic::common::v1::{InstrumentationScope, any_value};
    use opentelemetry_proto::tonic::resource::v1::Resource;
    use opentelemetry_proto::tonic::trace::v1::{ScopeSpans, Status};

    fn key_value(key: &str, value: any_value::Value) -> KeyValue {
        KeyValue {
            key: key.to_string(),
            value: Some(AnyValue { value: Some(value) }),
            ..Default::default()
        }
    }

    fn span(name: &str) -> Span {
        Span {
            trace_id: vec![0x11; 16],
            span_id: vec![0x22; 8],
            parent_span_id: vec![0x33; 8],
            trace_state: String::new(),
            flags: 0,
            name: name.to_string(),
            kind: SpanKind::Server as i32,
            start_time_unix_nano: 1_000_000_000,
            end_time_unix_nano: 1_500_000_000,
            attributes: vec![key_value(
                "http.method",
                any_value::Value::StringValue("GET".to_string()),
            )],
            dropped_attributes_count: 0,
            events: vec![],
            dropped_events_count: 0,
            links: vec![],
            dropped_links_count: 0,
            status: Some(Status {
                message: "boom".to_string(),
                code: StatusCode::Error as i32,
            }),
        }
    }

    fn resource_spans(service_name: Option<&str>) -> ResourceSpans {
        ResourceSpans {
            resource: Some(Resource {
                attributes: service_name
                    .map(|name| {
                        vec![key_value(
                            "service.name",
                            any_value::Value::StringValue(name.to_string()),
                        )]
                    })
                    .unwrap_or_default(),
                dropped_attributes_count: 0,
                entity_refs: vec![],
            }),
            scope_spans: vec![ScopeSpans {
                scope: Some(InstrumentationScope::default()),
                spans: vec![span("GET /realms/{realm}")],
                schema_url: String::new(),
            }],
            schema_url: String::new(),
        }
    }

    fn ids() -> (OrganisationId, DeploymentId) {
        (
            OrganisationId::new("11111111-1111-1111-1111-111111111111"),
            DeploymentId::new("22222222-2222-2222-2222-222222222222"),
        )
    }

    #[test]
    fn a_span_carries_the_resolved_tenancy_and_its_own_ids_hex_encoded() {
        let (organisation_id, deployment_id) = ids();
        let documents = SpanDocument::from_resource_spans(
            &resource_spans(Some("ferriskey-api")),
            &organisation_id,
            &deployment_id,
        );

        assert_eq!(documents.len(), 1);
        let document = &documents[0];
        assert_eq!(document.organisation_id, organisation_id);
        assert_eq!(document.deployment_id, deployment_id);
        assert_eq!(document.trace_id, "11".repeat(16));
        assert_eq!(document.span_id, "22".repeat(8));
        assert_eq!(document.parent_span_id, "33".repeat(8));
        assert_eq!(document.service_name, "ferriskey-api");
        assert_eq!(document.name, "GET /realms/{realm}");
        assert_eq!(document.kind, "server");
        assert_eq!(document.status_code, "error");
        assert_eq!(document.status_message, "boom");
    }

    #[test]
    fn duration_is_the_gap_between_start_and_end() {
        let (organisation_id, deployment_id) = ids();
        let document = &SpanDocument::from_resource_spans(
            &resource_spans(Some("svc")),
            &organisation_id,
            &deployment_id,
        )[0];

        assert_eq!(document.duration_nanos, 500_000_000);
    }

    #[test]
    fn a_resource_with_no_service_name_attribute_falls_back_to_unknown() {
        let (organisation_id, deployment_id) = ids();
        let document = &SpanDocument::from_resource_spans(
            &resource_spans(None),
            &organisation_id,
            &deployment_id,
        )[0];

        assert_eq!(document.service_name, UNKNOWN_SERVICE);
    }

    #[test]
    fn attributes_survive_as_nested_json_rather_than_flattened_columns() {
        let (organisation_id, deployment_id) = ids();
        let document = &SpanDocument::from_resource_spans(
            &resource_spans(Some("svc")),
            &organisation_id,
            &deployment_id,
        )[0];

        assert_eq!(
            document.attributes["http.method"],
            Value::String("GET".to_string())
        );
    }

    #[test]
    fn every_resource_spans_in_an_export_is_attributed_to_the_same_deployment() {
        let (organisation_id, deployment_id) = ids();
        let request = ExportTraceServiceRequest {
            resource_spans: vec![resource_spans(Some("a")), resource_spans(Some("b"))],
        };

        let documents = from_export_request(&request, &organisation_id, &deployment_id);

        assert_eq!(documents.len(), 2);
        assert!(
            documents
                .iter()
                .all(|doc| doc.deployment_id == deployment_id)
        );
        assert_eq!(documents[0].service_name, "a");
        assert_eq!(documents[1].service_name, "b");
    }
}
