use std::collections::BTreeSet;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use serde::Serialize;

use crate::store::types::TraceId;
use crate::store::Store;

#[derive(Serialize)]
pub struct TraceSummary {
    trace_id: String,
    span_count: usize,
    /// `None` for an empty trace or on an unrepresentable (malformed) timestamp pair — see
    /// `Trace::duration_nanos`.
    duration_nanos: Option<u64>,
    services: Vec<String>,
}

pub async fn list_traces(State(store): State<Arc<RwLock<Store>>>) -> Json<Vec<TraceSummary>> {
    let guard = store.read();
    let summaries: Vec<TraceSummary> = guard
        .traces()
        .map(|(id, trace)| {
            let mut services = BTreeSet::new();
            for idx in 0..trace.span_count() {
                if let Some(service_id) = trace.service_name_id(idx) {
                    services.insert(guard.service_name(service_id).to_string());
                }
            }
            TraceSummary {
                trace_id: id.to_hex(),
                span_count: trace.span_count(),
                duration_nanos: trace.duration_nanos(),
                services: services.into_iter().collect(),
            }
        })
        .collect();
    drop(guard);

    Json(summaries)
}

/// Columnar JSON + string table (§6). Deliberately **no attributes field** -- this is the
/// 40k-span path and the payload-discipline claim (acceptance #3); attributes are a separate
/// fetch-on-click route.
#[derive(Serialize)]
pub struct TracePayload {
    trace_id: String,
    generation: u32,
    string_table: Vec<String>,
    name_idx: Vec<u32>,
    parent_idx: Vec<Option<usize>>,
    start_time_unix_nano: Vec<u64>,
    end_time_unix_nano: Vec<u64>,
    status_message_idx: Vec<u32>,
    status_code: Vec<i32>,
    orphan: Vec<bool>,
    unknown_service: Vec<bool>,
    service_idx: Vec<Option<u32>>,
}

/// Dedupes repeated strings (span names, status messages, service names) into one shared table
/// referenced by index, instead of repeating them per span.
#[derive(Default)]
struct StringTable {
    strings: Vec<String>,
    index: FxHashMap<String, u32>,
}

impl StringTable {
    fn intern(&mut self, s: &str) -> u32 {
        if let Some(&id) = self.index.get(s) {
            return id;
        }
        let id = self.strings.len() as u32;
        self.strings.push(s.to_string());
        self.index.insert(s.to_string(), id);
        id
    }
}

pub async fn get_trace(State(store): State<Arc<RwLock<Store>>>, Path(id): Path<String>) -> Response {
    let Some(trace_id) = TraceId::from_hex(&id) else {
        return (StatusCode::BAD_REQUEST, "trace id must be 32 hex characters").into_response();
    };

    let guard = store.read();
    let Some(trace) = guard.trace(trace_id) else {
        drop(guard);
        return StatusCode::NOT_FOUND.into_response();
    };

    let n = trace.span_count();
    let mut table = StringTable::default();
    let mut name_idx = Vec::with_capacity(n);
    let mut parent_idx = Vec::with_capacity(n);
    let mut start_time_unix_nano = Vec::with_capacity(n);
    let mut end_time_unix_nano = Vec::with_capacity(n);
    let mut status_message_idx = Vec::with_capacity(n);
    let mut status_code = Vec::with_capacity(n);
    let mut orphan = Vec::with_capacity(n);
    let mut unknown_service = Vec::with_capacity(n);
    let mut service_idx = Vec::with_capacity(n);

    for idx in 0..n {
        name_idx.push(table.intern(trace.name(idx)));
        parent_idx.push(trace.parent_idx(idx));
        start_time_unix_nano.push(trace.start_time_unix_nano(idx));
        end_time_unix_nano.push(trace.end_time_unix_nano(idx));
        status_message_idx.push(table.intern(trace.status_message(idx)));
        status_code.push(trace.status_code(idx));
        orphan.push(trace.is_orphan(idx));
        unknown_service.push(trace.unknown_service(idx));
        service_idx.push(trace.service_name_id(idx).map(|sid| table.intern(guard.service_name(sid))));
    }

    let payload = TracePayload {
        trace_id: trace.trace_id.to_hex(),
        generation: trace.generation,
        string_table: table.strings,
        name_idx,
        parent_idx,
        start_time_unix_nano,
        end_time_unix_nano,
        status_message_idx,
        status_code,
        orphan,
        unknown_service,
        service_idx,
    };
    drop(guard);

    Json(payload).into_response()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    use super::*;
    use crate::store::trace::NewSpan;
    use crate::store::types::SpanId;

    fn store_with_one_trace() -> super::super::ApiState {
        let mut store = Store::new(10_000_000);
        let mut span = NewSpan {
            span_id: SpanId([2; 8]),
            parent_span_id: None,
            name: "root".to_string(),
            start_time_unix_nano: 100,
            end_time_unix_nano: 500,
            status_message: String::new(),
            status_code: 0,
            unknown_service: false,
            service_name: Some("checkout".to_string()),
            attributes: vec![("http.status_code".to_string(), crate::store::types::AttrValue::Int(200))],
        };
        store.insert_span(TraceId([1; 16]), span, 0);
        span = NewSpan {
            span_id: SpanId([3; 8]),
            parent_span_id: Some(SpanId([2; 8])),
            name: "child".to_string(),
            start_time_unix_nano: 150,
            end_time_unix_nano: 900,
            status_message: String::new(),
            status_code: 0,
            unknown_service: false,
            service_name: Some("db".to_string()),
            attributes: Vec::new(),
        };
        store.insert_span(TraceId([1; 16]), span, 0);

        super::super::test_state(store)
    }

    async fn json_body(response: Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn list_traces_reports_span_count_duration_and_services() {
        let app = super::super::router(store_with_one_trace());

        let response = app
            .oneshot(Request::builder().uri("/api/traces").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = json_body(response).await;
        let summaries = body.as_array().unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0]["span_count"], 2);
        assert_eq!(summaries[0]["duration_nanos"], 800); // 900 - 100
        assert_eq!(summaries[0]["services"], serde_json::json!(["checkout", "db"]));
    }

    #[tokio::test]
    async fn get_trace_payload_never_contains_attributes() {
        let app = super::super::router(store_with_one_trace());
        let hex = TraceId([1; 16]).to_hex();

        let response = app
            .oneshot(Request::builder().uri(format!("/api/traces/{hex}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let raw = String::from_utf8(bytes.to_vec()).unwrap();

        // acceptance #3: assert on the payload, not the response struct shape -- a stray
        // #[serde(flatten)] elsewhere would still be caught by this.
        assert!(!raw.contains("http.status_code"), "payload must not carry span attributes: {raw}");
        assert!(!raw.contains("200"), "the attribute value must not leak either: {raw}");
    }

    #[tokio::test]
    async fn get_trace_payload_is_columnar_with_a_shared_string_table() {
        let app = super::super::router(store_with_one_trace());
        let hex = TraceId([1; 16]).to_hex();

        let response = app
            .oneshot(Request::builder().uri(format!("/api/traces/{hex}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = json_body(response).await;

        assert_eq!(body["name_idx"].as_array().unwrap().len(), 2);
        let table = body["string_table"].as_array().unwrap();
        assert!(table.iter().any(|v| v == "root"));
        assert!(table.iter().any(|v| v == "child"));
        assert!(table.iter().any(|v| v == "checkout"));
        assert!(table.iter().any(|v| v == "db"));
    }

    #[tokio::test]
    async fn get_trace_returns_404_for_an_unknown_trace() {
        let app = super::super::router(store_with_one_trace());
        let hex = TraceId([9; 16]).to_hex();

        let response = app
            .oneshot(Request::builder().uri(format!("/api/traces/{hex}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn get_trace_returns_400_for_a_malformed_id() {
        let app = super::super::router(store_with_one_trace());

        let response = app
            .oneshot(Request::builder().uri("/api/traces/not-hex").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
