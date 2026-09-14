use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use parking_lot::RwLock;
use tokio::sync::mpsc;

use super::convert::{convert, RejectReason, SpanBatch};
use super::receiver_state::{ReceiverState, Transport};

/// Transport-agnostic outcome of the shared accept-or-reject decision (§2 — "so a wrong-port
/// test and a right-port test can share expectations"). Each transport maps this to its own wire
/// error: `tonic::Status` for gRPC, an HTTP status + body for axum.
#[derive(Debug)]
pub enum IngestError {
    Reject(RejectReason),
    /// A panic inside `convert()` was caught at the handler boundary (§2 hardening #3) instead of
    /// taking the process down.
    ConversionPanicked,
    /// The writer channel stayed full past `--ingest-timeout` (§4) — retryable backpressure.
    Backpressure,
    /// The writer task is gone (e.g. it panicked). Retrying will not help.
    WriterGone,
}

/// Everything a transport handler needs to run the shared ingest path. Cheap to clone — every
/// field is a `Sender`/`Arc`/`Copy` — so it doubles as axum router state directly.
#[derive(Clone)]
pub struct IngestState {
    tx: mpsc::Sender<SpanBatch>,
    receiver: Arc<RwLock<ReceiverState>>,
    ingest_timeout: Duration,
}

impl IngestState {
    pub fn new(tx: mpsc::Sender<SpanBatch>, receiver: Arc<RwLock<ReceiverState>>, ingest_timeout: Duration) -> Self {
        Self { tx, receiver, ingest_timeout }
    }

    /// Runs the shared accept-or-reject decision for an already-decoded request: `convert()`
    /// behind a panic boundary, envelope rejects recorded to the Receiver's ring buffer, then an
    /// accepted batch handed to the writer with `--ingest-timeout` backpressure. `transport`
    /// tags a successful arrival for the Receiver's per-protocol counts (§7).
    pub async fn accept(&self, request: ExportTraceServiceRequest, transport: Transport) -> Result<(), IngestError> {
        let batch = match catch_conversion_panic(|| convert(request)) {
            Ok(Ok(batch)) => batch,
            Ok(Err(reason)) => {
                self.record_reject(format!("{reason:?}"));
                return Err(IngestError::Reject(reason));
            }
            Err(()) => {
                self.record_reject("panic during conversion".to_string());
                return Err(IngestError::ConversionPanicked);
            }
        };

        let span_count = batch.spans.len() as u64;
        match tokio::time::timeout(self.ingest_timeout, self.tx.send(batch)).await {
            Ok(Ok(())) => {
                self.receiver.write().arrivals.record(transport, span_count);
                Ok(())
            }
            Ok(Err(_)) => Err(IngestError::WriterGone),
            Err(_) => Err(IngestError::Backpressure),
        }
    }

    fn record_reject(&self, reason: String) {
        self.receiver.write().rejects.push(reason, now_unix_nano());
    }
}

/// §2 hardening #3's `catch_unwind` boundary, pulled out as a named seam so a test can hand it a
/// closure that panics on purpose -- crafting a proto payload that naturally panics `convert()`
/// would be a rabbit hole, and isn't what this boundary is actually guarding.
fn catch_conversion_panic<F, R>(f: F) -> Result<R, ()>
where
    F: FnOnce() -> R,
{
    catch_unwind(AssertUnwindSafe(f)).map_err(|_| ())
}

fn now_unix_nano() -> u64 {
    // note: same pre-1970-clock fallback as store/writer.rs's now_unix_nano — cannot happen
    // on real hardware this tool runs on.
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span, Status};

    fn state_with_capacity(capacity: usize, ingest_timeout: Duration) -> (IngestState, mpsc::Receiver<SpanBatch>) {
        let (tx, rx) = mpsc::channel(capacity);
        let receiver = Arc::new(RwLock::new(ReceiverState::default()));
        (IngestState::new(tx, receiver, ingest_timeout), rx)
    }

    fn span_request() -> ExportTraceServiceRequest {
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
                        status: Some(Status { message: String::new(), code: 0 }),
                    }],
                    schema_url: String::new(),
                }],
                schema_url: String::new(),
            }],
        }
    }

    #[tokio::test]
    async fn accepted_batch_reaches_the_writer_channel() {
        let (state, mut rx) = state_with_capacity(8, Duration::from_secs(1));

        state.accept(span_request(), Transport::GrpcV4317).await.unwrap();

        let batch = rx.recv().await.unwrap();
        assert_eq!(batch.spans.len(), 1);
    }

    #[tokio::test]
    async fn accepted_batch_bumps_the_arrival_count_for_its_transport() {
        let (state, _rx) = state_with_capacity(8, Duration::from_secs(1));

        state.accept(span_request(), Transport::HttpJsonV4318).await.unwrap();

        let receiver = state.receiver.read();
        assert_eq!(receiver.arrivals.http_json_v4318, 1);
        assert_eq!(receiver.arrivals.grpc_v4317, 0);
    }

    #[tokio::test]
    async fn empty_payload_is_rejected_and_recorded() {
        let (state, _rx) = state_with_capacity(8, Duration::from_secs(1));

        let err = state
            .accept(ExportTraceServiceRequest { resource_spans: vec![] }, Transport::GrpcV4317)
            .await
            .unwrap_err();
        assert!(matches!(err, IngestError::Reject(RejectReason::EmptyPayload)));
        assert_eq!(state.receiver.read().rejects.len(), 1);
    }

    #[test]
    fn catch_conversion_panic_passes_through_a_normal_result() {
        assert_eq!(catch_conversion_panic(|| 42), Ok(42));
    }

    #[test]
    fn catch_conversion_panic_maps_a_panic_to_err_instead_of_unwinding() {
        // suppress the default panic-hook println for this expected, intentional panic
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let result = catch_conversion_panic(|| -> () { panic!("boom") });
        std::panic::set_hook(previous_hook);

        assert_eq!(result, Err(()));
    }

    #[tokio::test]
    async fn a_full_channel_times_out_as_backpressure_not_a_hang() {
        // capacity 1: the first send fills the channel; nothing ever drains it, so the second
        // blocks until the timeout fires.
        let (state, _rx) = state_with_capacity(1, Duration::from_millis(10));
        state.accept(span_request(), Transport::GrpcV4317).await.unwrap();

        let err = state.accept(span_request(), Transport::GrpcV4317).await.unwrap_err();
        assert!(matches!(err, IngestError::Backpressure));
    }
}
