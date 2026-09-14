use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use parking_lot::RwLock;
use serde::Serialize;

use crate::ingest::receiver_state::{ArrivalCounts, RejectRecord, ReceiverState};
use crate::receiver::Counters;
use crate::store::Store;

/// `GET /api/receiver` (§6, §7). "Positive signal first" -- arrivals and counters lead the
/// payload, `recent_rejects` (oldest-first, as the ring buffer holds them) trails it.
#[derive(Serialize)]
pub struct ReceiverResponse {
    arrivals: ArrivalCounts,
    counters: Counters,
    recent_rejects: Vec<RejectRecord>,
}

pub async fn get_receiver(
    State(store): State<Arc<RwLock<Store>>>,
    State(receiver): State<Arc<RwLock<ReceiverState>>>,
) -> Json<ReceiverResponse> {
    let store_guard = store.read();
    let counters = store_guard.counters;
    drop(store_guard);

    let receiver_guard = receiver.read();
    let arrivals = receiver_guard.arrivals;
    let recent_rejects: Vec<RejectRecord> = receiver_guard.rejects.recent().cloned().collect();
    drop(receiver_guard);

    Json(ReceiverResponse { arrivals, counters, recent_rejects })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    use super::*;
    use crate::ingest::receiver_state::Transport;
    use crate::store::trace::NewSpan;
    use crate::store::types::{SpanId, TraceId};

    fn empty_state() -> super::super::ApiState {
        super::super::ApiState {
            store: Arc::new(RwLock::new(Store::new(10_000_000))),
            receiver: Arc::new(RwLock::new(ReceiverState::default())),
        }
    }

    #[tokio::test]
    async fn reports_store_counters_arrivals_and_recent_rejects() {
        let state = empty_state();
        state.store.write().insert_span(
            TraceId([1; 16]),
            NewSpan {
                span_id: SpanId([1; 8]),
                parent_span_id: None,
                name: "op".to_string(),
                start_time_unix_nano: 0,
                end_time_unix_nano: 0,
                status_message: String::new(),
                status_code: 0,
                unknown_service: true,
                service_name: None,
                attributes: Vec::new(),
            },
            0,
        );
        // dedupe overwrite bumps duplicate_span
        state.store.write().insert_span(
            TraceId([1; 16]),
            NewSpan {
                span_id: SpanId([1; 8]),
                parent_span_id: None,
                name: "op-renamed".to_string(),
                start_time_unix_nano: 0,
                end_time_unix_nano: 0,
                status_message: String::new(),
                status_code: 0,
                unknown_service: true,
                service_name: None,
                attributes: Vec::new(),
            },
            1,
        );
        state.receiver.write().arrivals.record(Transport::HttpProtobufV4318, 2);
        state.receiver.write().rejects.push("EmptyPayload".to_string(), 5);

        let app = super::super::router(state);
        let response =
            app.oneshot(Request::builder().uri("/api/receiver").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(body["arrivals"]["http_protobuf_v4318"], 2);
        assert_eq!(body["counters"]["duplicate_span"], 1);
        assert_eq!(body["recent_rejects"].as_array().unwrap().len(), 1);
        assert_eq!(body["recent_rejects"][0]["reason"], "EmptyPayload");
    }
}
