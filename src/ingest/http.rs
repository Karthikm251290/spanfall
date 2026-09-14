use axum::body::Bytes;
use axum::extract::State;
use axum::http::header::{CONTENT_TYPE, RETRY_AFTER};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;

use super::convert::{decode, ContentType, RejectReason};
use super::handler::{IngestError, IngestState};
use super::receiver_state::Transport;

/// OTLP/HTTP on :4318, both protobuf and JSON bodies (§3). Only this transport calls `decode()`
/// — gRPC arrives pre-decoded via tonic.
pub fn router(state: IngestState) -> Router {
    Router::new().route("/v1/traces", post(export)).with_state(state)
}

async fn export(State(state): State<IngestState>, headers: axum::http::HeaderMap, body: Bytes) -> Response {
    let (content_type, transport) = match headers.get(CONTENT_TYPE).and_then(|v| v.to_str().ok()) {
        Some(ct) if ct.starts_with("application/json") => (ContentType::Json, Transport::HttpJsonV4318),
        // OTLP/HTTP protobuf exporters send application/x-protobuf; default to protobuf for any
        // other/missing content-type rather than reject on that alone.
        _ => (ContentType::Protobuf, Transport::HttpProtobufV4318),
    };

    let request = match decode(&body, content_type) {
        Ok(req) => req,
        Err(reason) => return (StatusCode::BAD_REQUEST, format!("{reason:?}")).into_response(),
    };

    match state.accept(request, transport).await {
        Ok(()) => StatusCode::OK.into_response(),
        // EmptyPayload (§2) is a named, expected condition, not a wire failure.
        Err(IngestError::Reject(RejectReason::EmptyPayload)) => StatusCode::OK.into_response(),
        Err(IngestError::Reject(reason)) => (StatusCode::BAD_REQUEST, format!("{reason:?}")).into_response(),
        Err(IngestError::ConversionPanicked) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        Err(IngestError::Backpressure) => {
            (StatusCode::TOO_MANY_REQUESTS, [(RETRY_AFTER, "1")]).into_response()
        }
        Err(IngestError::WriterGone) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use axum::body::Body;
    use axum::http::Request;
    use parking_lot::RwLock;
    use prost::Message;
    use tokio::sync::mpsc;
    use tower::ServiceExt;

    use super::*;
    use crate::ingest::receiver_state::ReceiverState;
    use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
    use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span, Status as PbStatus};

    fn test_router(capacity: usize) -> (Router, mpsc::Receiver<crate::ingest::convert::SpanBatch>) {
        let (tx, rx) = mpsc::channel(capacity);
        let receiver = Arc::new(RwLock::new(ReceiverState::default()));
        let state = IngestState::new(tx, receiver, Duration::from_secs(1));
        (router(state), rx)
    }

    fn one_span_request() -> ExportTraceServiceRequest {
        ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: None,
                scope_spans: vec![ScopeSpans {
                    scope: None,
                    spans: vec![Span {
                        trace_id: vec![1; 16],
                        span_id: vec![2; 8],
                        trace_state: String::new(),
                        parent_span_id: Vec::new(),
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
                        status: Some(PbStatus { message: String::new(), code: 0 }),
                    }],
                    schema_url: String::new(),
                }],
                schema_url: String::new(),
            }],
        }
    }

    #[tokio::test]
    async fn protobuf_body_is_accepted_and_reaches_the_writer() {
        let (app, mut rx) = test_router(8);
        let body = one_span_request().encode_to_vec();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/traces")
                    .header(CONTENT_TYPE, "application/x-protobuf")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let batch = rx.recv().await.unwrap();
        assert_eq!(batch.spans.len(), 1);
    }

    #[tokio::test]
    async fn json_body_is_accepted_and_reaches_the_writer() {
        let (app, mut rx) = test_router(8);
        let body = serde_json::to_vec(&one_span_request()).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/traces")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let batch = rx.recv().await.unwrap();
        assert_eq!(batch.spans.len(), 1);
    }

    #[tokio::test]
    async fn truncated_protobuf_body_is_a_400_not_a_panic() {
        let (app, _rx) = test_router(8);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/traces")
                    .header(CONTENT_TYPE, "application/x-protobuf")
                    .body(Body::from(vec![0xFF, 0xFF, 0xFF]))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn empty_payload_is_a_200_not_a_reject() {
        let (app, _rx) = test_router(8);
        let body = ExportTraceServiceRequest { resource_spans: vec![] }.encode_to_vec();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/traces")
                    .header(CONTENT_TYPE, "application/x-protobuf")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
