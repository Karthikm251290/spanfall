//! Black-box, end-to-end coverage through the real writer and store -- not the store-level unit
//! test alone -- for acceptance rows that only mean something wired together (spec 01 §Acceptance).

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span, Status as PbStatus};
use parking_lot::RwLock;
use prost::Message;
use tower::ServiceExt;

use spanfall::api::{self, ApiState};
use spanfall::ingest::handler::IngestState;
use spanfall::ingest::http as ingest_http;
use spanfall::ingest::receiver_state::ReceiverState;
use spanfall::store::writer::spawn_writer;
use spanfall::store::Store;

fn one_span_request(trace_id: [u8; 16], span_id: [u8; 8]) -> ExportTraceServiceRequest {
    ExportTraceServiceRequest {
        resource_spans: vec![ResourceSpans {
            resource: None,
            scope_spans: vec![ScopeSpans {
                scope: None,
                spans: vec![Span {
                    trace_id: trace_id.to_vec(),
                    span_id: span_id.to_vec(),
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

/// Acceptance #6: a retried batch -- the same span sent twice, exactly what an exporter does
/// after a 429 -- must not double the span count. It dedupes in place and bumps `duplicate_span`.
#[tokio::test]
async fn retried_batch_dedupes_span_count_and_bumps_duplicate_span_counter() {
    let store = Arc::new(RwLock::new(Store::new(10_000_000)));
    let (tx, events_seq, _writer_handle) = spawn_writer(store.clone(), 8);
    let receiver = Arc::new(RwLock::new(ReceiverState::default()));
    let ingest_state = IngestState::new(tx, receiver.clone(), Duration::from_secs(1));
    let ingest_router = ingest_http::router(ingest_state);
    let api_state = ApiState { store: store.clone(), receiver, events_seq };

    let body = one_span_request([9; 16], [7; 8]).encode_to_vec();

    for _ in 0..2 {
        let response = ingest_router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/traces")
                    .header("content-type", "application/x-protobuf")
                    .body(Body::from(body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // The writer applies batches asynchronously off the channel -- poll rather than assume both
    // sends have already landed by the time the HTTP responses came back.
    let mut duplicate_span = 0;
    for _ in 0..50 {
        duplicate_span = store.read().counters.duplicate_span;
        if duplicate_span >= 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(duplicate_span, 1, "second send of the same span id must dedupe, not double-count");

    let api_router = api::router(api_state);
    let response =
        api_router.oneshot(Request::builder().uri("/api/traces").body(Body::empty()).unwrap()).await.unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let traces: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(traces.as_array().unwrap().len(), 1, "one trace, not two");
    assert_eq!(traces[0]["span_count"], 1, "span count unchanged -- the retry deduped in place");
}
