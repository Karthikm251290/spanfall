use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::common::v1::{any_value::Value as PbValue, KeyValue};
use opentelemetry_proto::tonic::trace::v1::ResourceSpans;
use prost::Message;

use crate::store::trace::NewSpan;
use crate::store::types::{AttrValue, SpanId, TraceId};

/// Envelope-level failures (§2). Per-span problems never reach this type — they are counted and
/// the span is still accepted (see `SpanBatch::malformed_span_count` and `.partial`).
#[derive(Debug, PartialEq)]
pub enum RejectReason {
    /// The exporter connected but sent zero spans — the sampler dropped everything. A named
    /// first-run failure mode, must be reported rather than shown as a silent blank screen.
    EmptyPayload,
    DecodeError(String),
}

pub enum ContentType {
    Protobuf,
    Json,
}

#[derive(Debug)]
pub struct SpanBatch {
    pub spans: Vec<(TraceId, NewSpan)>,
    /// PartialBatch (§2): some spans in the batch converted and some did not.
    pub partial: bool,
    pub malformed_span_count: usize,
}

/// Decodes the wire bytes into the shared proto request type. Only the HTTP transport needs
/// this: gRPC via tonic decodes protobuf itself before the handler ever runs.
pub fn decode(bytes: &[u8], content_type: ContentType) -> Result<ExportTraceServiceRequest, RejectReason> {
    match content_type {
        ContentType::Protobuf => ExportTraceServiceRequest::decode(bytes)
            .map_err(|e| RejectReason::DecodeError(e.to_string())),
        ContentType::Json => {
            serde_json::from_slice(bytes).map_err(|e| RejectReason::DecodeError(e.to_string()))
        }
    }
}

/// The one shared semantic conversion both transports call once they have a decoded request
/// (§2 — "so a wrong-port test and a right-port test can share expectations").
pub fn convert(request: ExportTraceServiceRequest) -> Result<SpanBatch, RejectReason> {
    let mut spans = Vec::new();
    let mut malformed_span_count = 0usize;
    let mut saw_any_span = false;

    for resource_spans in &request.resource_spans {
        let service_name = service_name_of(resource_spans);
        let unknown_service = service_name.is_none();

        for scope_spans in &resource_spans.scope_spans {
            for pb_span in &scope_spans.spans {
                saw_any_span = true;

                let Some(trace_id) = trace_id_of(&pb_span.trace_id) else {
                    malformed_span_count += 1;
                    continue;
                };
                let Some(span_id) = span_id_of(&pb_span.span_id) else {
                    malformed_span_count += 1;
                    continue;
                };
                let parent_span_id = span_id_of(&pb_span.parent_span_id);

                let attributes = pb_span
                    .attributes
                    .iter()
                    .filter_map(attr_of)
                    .collect();

                let status_message = pb_span
                    .status
                    .as_ref()
                    .map(|s| s.message.clone())
                    .unwrap_or_default();
                let status_code = pb_span.status.as_ref().map(|s| s.code).unwrap_or(0);

                let new_span = NewSpan {
                    span_id,
                    parent_span_id,
                    name: pb_span.name.clone(),
                    start_time_unix_nano: pb_span.start_time_unix_nano,
                    end_time_unix_nano: pb_span.end_time_unix_nano,
                    status_message,
                    status_code,
                    unknown_service,
                    service_name: service_name.clone(),
                    attributes,
                };
                spans.push((trace_id, new_span));
            }
        }
    }

    if !saw_any_span {
        return Err(RejectReason::EmptyPayload);
    }

    Ok(SpanBatch {
        partial: malformed_span_count > 0,
        malformed_span_count,
        spans,
    })
}

fn service_name_of(resource_spans: &ResourceSpans) -> Option<String> {
    let resource = resource_spans.resource.as_ref()?;
    resource.attributes.iter().find_map(|kv| {
        if kv.key != "service.name" {
            return None;
        }
        match kv.value.as_ref()?.value.as_ref()? {
            PbValue::StringValue(s) => Some(s.clone()),
            _ => None,
        }
    })
}

fn trace_id_of(bytes: &[u8]) -> Option<TraceId> {
    let arr: [u8; 16] = bytes.try_into().ok()?;
    Some(TraceId(arr))
}

fn span_id_of(bytes: &[u8]) -> Option<SpanId> {
    let arr: [u8; 8] = bytes.try_into().ok()?;
    Some(SpanId(arr))
}

/// Unsupported attribute value kinds (array/kvlist/bytes/unset) are dropped rather than mapped.
///
/// ponytail: `AttrValue` has no array/nested-object variant. Every real-world attribute this
/// tool exists to diagnose (http.status_code, service.name, ...) is a scalar; add a variant if
/// a real trace needs one.
fn attr_of(kv: &KeyValue) -> Option<(String, AttrValue)> {
    let value = match kv.value.as_ref()?.value.as_ref()? {
        PbValue::StringValue(s) => AttrValue::Str(s.clone()),
        PbValue::IntValue(i) => AttrValue::Int(*i),
        PbValue::DoubleValue(d) => AttrValue::Double(*d),
        PbValue::BoolValue(b) => AttrValue::Bool(*b),
        PbValue::ArrayValue(_) | PbValue::KvlistValue(_) | PbValue::BytesValue(_) => return None,
        // Profiling-signal-only string interning; never populated by trace exporters.
        PbValue::StringValueStrindex(_) => return None,
    };
    Some((kv.key.clone(), value))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use opentelemetry_proto::tonic::common::v1::{any_value::Value, AnyValue, InstrumentationScope};
    use opentelemetry_proto::tonic::resource::v1::Resource;
    use opentelemetry_proto::tonic::trace::v1::{ScopeSpans, Span, Status};

    fn kv(key: &str, value: Value) -> KeyValue {
        KeyValue {
            key: key.to_string(),
            value: Some(AnyValue { value: Some(value) }),
            key_strindex: 0,
        }
    }

    fn minimal_span(trace_id: [u8; 16], span_id: [u8; 8], parent: Option<[u8; 8]>) -> Span {
        Span {
            trace_id: trace_id.to_vec(),
            span_id: span_id.to_vec(),
            trace_state: String::new(),
            parent_span_id: parent.map(|p| p.to_vec()).unwrap_or_default(),
            flags: 0,
            name: "op".to_string(),
            kind: 0,
            start_time_unix_nano: 1,
            end_time_unix_nano: 2,
            attributes: Vec::new(),
            dropped_attributes_count: 0,
            events: Vec::new(),
            dropped_events_count: 0,
            links: Vec::new(),
            dropped_links_count: 0,
            status: Some(Status { message: String::new(), code: 0 }),
        }
    }

    fn request_with(resource_spans: Vec<ResourceSpans>) -> ExportTraceServiceRequest {
        ExportTraceServiceRequest { resource_spans }
    }

    fn resource_spans_with(service_name: Option<&str>, spans: Vec<Span>) -> ResourceSpans {
        let resource = service_name.map(|name| Resource {
            attributes: vec![kv("service.name", Value::StringValue(name.to_string()))],
            dropped_attributes_count: 0,
            entity_refs: Vec::new(),
        });
        ResourceSpans {
            resource,
            scope_spans: vec![ScopeSpans {
                scope: Some(InstrumentationScope::default()),
                spans,
                schema_url: String::new(),
            }],
            schema_url: String::new(),
        }
    }

    #[test]
    fn zero_resource_spans_is_empty_payload() {
        let req = request_with(vec![]);
        assert_eq!(convert(req).unwrap_err(), RejectReason::EmptyPayload);
    }

    #[test]
    fn zero_spans_inside_present_resource_spans_is_also_empty_payload() {
        let req = request_with(vec![resource_spans_with(Some("checkout"), vec![])]);
        assert_eq!(convert(req).unwrap_err(), RejectReason::EmptyPayload);
    }

    #[test]
    fn a_well_formed_span_converts_with_known_service() {
        let span = minimal_span([1; 16], [2; 8], None);
        let req = request_with(vec![resource_spans_with(Some("checkout"), vec![span])]);

        let batch = convert(req).unwrap();
        assert!(!batch.partial);
        assert_eq!(batch.malformed_span_count, 0);
        assert_eq!(batch.spans.len(), 1);
        let (trace_id, new_span) = &batch.spans[0];
        assert_eq!(*trace_id, TraceId([1; 16]));
        assert_eq!(new_span.span_id, SpanId([2; 8]));
        assert!(!new_span.unknown_service);
        assert_eq!(new_span.service_name.as_deref(), Some("checkout"));
    }

    #[test]
    fn status_code_carries_through_from_the_proto() {
        let mut span = minimal_span([1; 16], [2; 8], None);
        span.status = Some(Status { message: "boom".to_string(), code: 2 });
        let req = request_with(vec![resource_spans_with(Some("svc"), vec![span])]);

        let batch = convert(req).unwrap();
        assert_eq!(batch.spans[0].1.status_code, 2);
        assert_eq!(batch.spans[0].1.status_message, "boom");
    }

    #[test]
    fn missing_service_name_flags_unknown_service_but_still_accepts_the_span() {
        let span = minimal_span([1; 16], [2; 8], None);
        let req = request_with(vec![resource_spans_with(None, vec![span])]);

        let batch = convert(req).unwrap();
        assert_eq!(batch.spans.len(), 1);
        assert!(batch.spans[0].1.unknown_service);
    }

    #[test]
    fn a_malformed_trace_id_drops_only_that_span_and_flags_partial() {
        let ok = minimal_span([1; 16], [2; 8], None);
        let mut bad = minimal_span([1; 16], [3; 8], None);
        bad.trace_id = vec![0xFF; 15]; // wrong length

        let req = request_with(vec![resource_spans_with(Some("svc"), vec![ok, bad])]);
        let batch = convert(req).unwrap();

        assert!(batch.partial);
        assert_eq!(batch.malformed_span_count, 1);
        assert_eq!(batch.spans.len(), 1);
    }

    #[test]
    fn parent_span_id_carries_through_when_present() {
        let span = minimal_span([1; 16], [2; 8], Some([9; 8]));
        let req = request_with(vec![resource_spans_with(Some("svc"), vec![span])]);

        let batch = convert(req).unwrap();
        assert_eq!(batch.spans[0].1.parent_span_id, Some(SpanId([9; 8])));
    }

    #[test]
    fn scalar_attribute_kinds_convert_and_unsupported_kinds_are_dropped() {
        let mut span = minimal_span([1; 16], [2; 8], None);
        span.attributes = vec![
            kv("s", Value::StringValue("x".to_string())),
            kv("i", Value::IntValue(7)),
            kv("d", Value::DoubleValue(1.5)),
            kv("b", Value::BoolValue(true)),
            kv("arr", Value::ArrayValue(Default::default())),
        ];
        let req = request_with(vec![resource_spans_with(Some("svc"), vec![span])]);

        let batch = convert(req).unwrap();
        assert_eq!(batch.spans[0].1.attributes.len(), 4);
    }

    #[test]
    fn protobuf_round_trip_decodes() {
        let span = minimal_span([1; 16], [2; 8], None);
        let req = request_with(vec![resource_spans_with(Some("svc"), vec![span])]);
        let bytes = req.encode_to_vec();

        let decoded = decode(&bytes, ContentType::Protobuf).unwrap();
        assert_eq!(decoded.resource_spans.len(), 1);
    }

    #[test]
    fn truncated_protobuf_is_a_typed_decode_error_not_a_panic() {
        let err = decode(&[0xFF, 0xFF, 0xFF], ContentType::Protobuf).unwrap_err();
        assert!(matches!(err, RejectReason::DecodeError(_)));
    }

    #[test]
    fn json_round_trip_decodes() {
        let span = minimal_span([1; 16], [2; 8], None);
        let req = request_with(vec![resource_spans_with(Some("svc"), vec![span])]);
        let bytes = serde_json::to_vec(&req).unwrap();

        let decoded = decode(&bytes, ContentType::Json).unwrap();
        assert_eq!(decoded.resource_spans.len(), 1);
    }

    #[test]
    fn malformed_json_is_a_typed_decode_error_not_a_panic() {
        let err = decode(b"{not json", ContentType::Json).unwrap_err();
        assert!(matches!(err, RejectReason::DecodeError(_)));
    }
}
