//! `list` and `--dump` (spec 07): both are thin clients against an already-running instance's
//! Read API -- resident traces only exist inside that process's in-memory `Store`, so a fresh
//! invocation has nothing of its own to report. `--dump` is spec 01 §8's M0 exit criterion: prove
//! spans, flags, rejects, and counters are all inspectable with no UI.

use crate::client;

/// Rows of `GET /api/traces`, for both `fetch_list` and as the trace-id source for `fetch_dump`.
fn traces(addr: &str) -> Result<Vec<serde_json::Value>, String> {
    let body = client::get_json(addr, "/api/traces").map_err(|e| e.0)?;
    body.as_array().cloned().ok_or_else(|| "unexpected /api/traces response shape".to_string())
}

pub fn fetch_list(addr: &str) -> Result<Vec<serde_json::Value>, String> {
    traces(addr)
}

pub fn print_list(addr: &str) -> Result<(), String> {
    let rows = fetch_list(addr)?;
    if rows.is_empty() {
        println!("no resident traces");
        return Ok(());
    }

    println!("{:<34} {:>6} {:>14}  SERVICES", "TRACE ID", "SPANS", "DURATION(ns)");
    for row in &rows {
        let trace_id = row["trace_id"].as_str().unwrap_or("?");
        let span_count = row["span_count"].as_u64().unwrap_or(0);
        let duration =
            row["duration_nanos"].as_u64().map_or_else(|| "-".to_string(), |d| d.to_string());
        let services: Vec<&str> =
            row["services"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
        println!("{trace_id:<34} {span_count:>6} {duration:>14}  {}", services.join(","));
    }
    Ok(())
}

/// The full per-trace payload plus the receiver's arrivals/counters/rejects -- a superset of
/// `/api/traces/:id` (which deliberately omits attributes, acceptance #3) is not needed here:
/// spec 01 §8 asks for spans, flags, rejects, and counters, all already on these two routes.
pub fn fetch_dump(addr: &str) -> Result<serde_json::Value, String> {
    let rows = traces(addr)?;
    let mut full_traces = Vec::with_capacity(rows.len());
    for row in &rows {
        let Some(trace_id) = row["trace_id"].as_str() else {
            return Err("unexpected /api/traces response shape".to_string());
        };
        let trace = client::get_json(addr, &format!("/api/traces/{trace_id}")).map_err(|e| e.0)?;
        full_traces.push(trace);
    }

    let receiver = client::get_json(addr, "/api/receiver").map_err(|e| e.0)?;

    Ok(serde_json::json!({ "traces": full_traces, "receiver": receiver }))
}

pub fn print_dump(addr: &str) -> Result<(), String> {
    let dump = fetch_dump(addr)?;
    let text = serde_json::to_string_pretty(&dump).map_err(|e| e.to_string())?;
    println!("{text}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::RwLock;
    use tokio::sync::watch;

    use super::*;
    use crate::api::{self, ApiState};
    use crate::ingest::receiver_state::ReceiverState;
    use crate::store::trace::NewSpan;
    use crate::store::types::{SpanId, TraceId};
    use crate::store::Store;

    /// Starts the real API router on an ephemeral loopback port and returns its address.
    async fn spawn_test_server(store: Store) -> String {
        let (_tx, events_seq) = watch::channel(0u64);
        let state = ApiState {
            store: Arc::new(RwLock::new(store)),
            receiver: Arc::new(RwLock::new(ReceiverState::default())),
            events_seq,
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local_addr").to_string();
        tokio::spawn(async move {
            let _ = axum::serve(listener, api::router(state)).await;
        });
        addr
    }

    fn store_with_one_trace() -> Store {
        let mut store = Store::new(10_000_000);
        store.insert_span(
            TraceId([1; 16]),
            NewSpan {
                span_id: SpanId([2; 8]),
                parent_span_id: None,
                name: "root".to_string(),
                start_time_unix_nano: 100,
                end_time_unix_nano: 500,
                status_message: String::new(),
                status_code: 0,
                unknown_service: false,
                service_name: Some("checkout".to_string()),
                attributes: Vec::new(),
            },
            0,
        );
        store
    }

    // multi_thread: fetch_* makes a blocking `std::net::TcpStream` call on the test's own thread,
    // which would starve a current-thread runtime and never let the spawned server task run.
    #[tokio::test(flavor = "multi_thread")]
    async fn fetch_list_returns_the_resident_trace_summaries() {
        let addr = spawn_test_server(store_with_one_trace()).await;

        let rows = fetch_list(&addr).unwrap_or_else(|e| panic!("{e}"));

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["span_count"], 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fetch_dump_carries_traces_and_receiver_counters() {
        let addr = spawn_test_server(store_with_one_trace()).await;

        let dump = fetch_dump(&addr).unwrap_or_else(|e| panic!("{e}"));

        let traces = dump["traces"].as_array().unwrap_or_else(|| panic!("no traces array: {dump}"));
        assert_eq!(traces.len(), 1);
        assert_eq!(traces[0]["name_idx"].as_array().unwrap_or_else(|| panic!("no name_idx")).len(), 1);
        assert!(dump["receiver"]["counters"].is_object(), "no receiver.counters in {dump}");
    }

    #[test]
    fn no_running_instance_is_a_clear_error_not_a_panic() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local_addr").to_string();
        drop(listener);

        let err = fetch_list(&addr).unwrap_err();
        assert!(err.contains("spanfall instance"), "unexpected message: {err}");
    }
}
