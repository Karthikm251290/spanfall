use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use parking_lot::RwLock;
use serde::Serialize;

use crate::store::types::{AttrValue, TraceId};
use crate::store::Store;

#[derive(Serialize)]
pub struct AttributeEntry {
    key: String,
    value: AttrValue,
}

/// One span's attributes (§6) -- "a slice out of the CSR arrays", fetched on click rather than
/// carried in the trace payload (§6's payload-discipline claim, acceptance #3).
pub async fn get_attributes(
    State(store): State<Arc<RwLock<Store>>>,
    Path((id, idx)): Path<(String, usize)>,
) -> Response {
    let Some(trace_id) = TraceId::from_hex(&id) else {
        return (StatusCode::BAD_REQUEST, "trace id must be 32 hex characters").into_response();
    };

    let guard = store.read();
    let Some(trace) = guard.trace(trace_id) else {
        drop(guard);
        return StatusCode::NOT_FOUND.into_response();
    };
    if idx >= trace.span_count() {
        drop(guard);
        return StatusCode::NOT_FOUND.into_response();
    }

    let entries: Vec<AttributeEntry> = trace
        .attribute_keys(idx)
        .iter()
        .zip(trace.attributes(idx).iter())
        .map(|(&key_id, value)| AttributeEntry {
            key: guard.attribute_key(key_id).to_string(),
            value: value.clone(),
        })
        .collect();
    drop(guard);

    Json(entries).into_response()
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

    fn store_with_one_span() -> super::super::ApiState {
        let mut store = Store::new(10_000_000);
        let span = NewSpan {
            span_id: SpanId([2; 8]),
            parent_span_id: None,
            name: "root".to_string(),
            start_time_unix_nano: 1,
            end_time_unix_nano: 2,
            status_message: String::new(),
            status_code: 0,
            unknown_service: false,
            service_name: Some("checkout".to_string()),
            attributes: vec![
                ("http.status_code".to_string(), AttrValue::Int(200)),
                ("http.method".to_string(), AttrValue::Str("GET".to_string())),
            ],
        };
        store.insert_span(TraceId([1; 16]), span, 0);
        super::super::test_state(store)
    }

    #[tokio::test]
    async fn returns_the_spans_key_value_attributes() {
        let app = super::super::router(store_with_one_span());
        let hex = TraceId([1; 16]).to_hex();

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/traces/{hex}/spans/0/attributes"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let entries = body.as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e["key"] == "http.status_code" && e["value"] == 200));
        assert!(entries.iter().any(|e| e["key"] == "http.method" && e["value"] == "GET"));
    }

    #[tokio::test]
    async fn out_of_range_span_index_is_404() {
        let app = super::super::router(store_with_one_span());
        let hex = TraceId([1; 16]).to_hex();

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/traces/{hex}/spans/99/attributes"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
