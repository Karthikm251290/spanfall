use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::Store;
use crate::ingest::convert::SpanBatch;

/// Spawns the single writer task that owns all store mutation (§4,
/// `01-m0-store-ingest.md`). Both transports decode in their own request handler (parallel
/// across cores) and hand the decoded batch here over a bounded channel; interning and
/// appending are serial and cheap, so they belong behind the channel rather than in the
/// handlers. `capacity` bounds the channel -- a full channel is what makes `send().await` block
/// and eventually time out at the handler (backpressure), never a silent drop.
pub fn spawn_writer(store: Arc<RwLock<Store>>, capacity: usize) -> (mpsc::Sender<SpanBatch>, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel(capacity);
    let handle = tokio::spawn(run(store, rx));
    (tx, handle)
}

async fn run(store: Arc<RwLock<Store>>, mut rx: mpsc::Receiver<SpanBatch>) {
    while let Some(batch) = rx.recv().await {
        let now = now_unix_nano();
        let malformed = batch.malformed_span_count as u64;

        // Hold the write lock for the duration of one batch's appends only -- microseconds --
        // never across a decode or a send.
        let mut guard = store.write();
        for (trace_id, span) in batch.spans {
            guard.insert_span(trace_id, span, now);
        }
        guard.counters.malformed_span += malformed;
    }
}

fn now_unix_nano() -> u64 {
    // ponytail: falls back to 0 on a pre-1970 clock, which cannot happen on real hardware this
    // tool runs on. Not worth a Result here.
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::trace::NewSpan;
    use crate::store::types::{SpanId, TraceId};

    fn span(id: u8) -> NewSpan {
        NewSpan {
            span_id: SpanId([id; 8]),
            parent_span_id: None,
            name: "op".to_string(),
            start_time_unix_nano: 0,
            end_time_unix_nano: 0,
            status_message: String::new(),
            status_code: 0,
            unknown_service: false,
            service_name: None,
            attributes: Vec::new(),
        }
    }

    fn batch(spans: Vec<(TraceId, NewSpan)>, malformed_span_count: usize) -> SpanBatch {
        SpanBatch { spans, partial: malformed_span_count > 0, malformed_span_count }
    }

    #[tokio::test]
    async fn writer_applies_batches_and_updates_counters() {
        let store = Arc::new(RwLock::new(Store::new(10_000_000)));
        let (tx, handle) = spawn_writer(store.clone(), 8);

        tx.send(batch(vec![(TraceId([1; 16]), span(1))], 0)).await.unwrap();
        tx.send(batch(vec![(TraceId([1; 16]), span(2))], 2)).await.unwrap();

        // dropping every sender closes the channel; the writer's `while let Some(..) = recv()`
        // then exits, so joining the handle deterministically waits for both batches to apply
        // -- no arbitrary sleep needed.
        drop(tx);
        handle.await.unwrap();

        let guard = store.read();
        assert_eq!(guard.trace(TraceId([1; 16])).unwrap().span_count(), 2);
        assert_eq!(guard.counters.malformed_span, 2);
    }
}
