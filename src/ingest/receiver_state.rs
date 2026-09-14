use std::collections::VecDeque;

// ponytail: no size pinned by spec 01 §7 for the rejects ring, only that it "wraps oldest-first
// at capacity" and that wraparound is a tested behaviour. 100 is a reasonable Receiver-panel
// window; bump if a real session needs more history.
const REJECT_RING_CAPACITY: usize = 100;

/// One envelope-level reject (§2's `EmptyPayload` / `DecodeError`), recorded for the Receiver
/// panel (§7) — "must be reported, not shown as a silent blank screen".
#[derive(Debug, Clone, serde::Serialize)]
pub struct RejectRecord {
    pub reason: String,
    pub at_unix_nano: u64,
}

/// Which listener accepted a batch (§3). Distinct from `ContentType` -- gRPC never goes through
/// `decode()` at all, tonic decodes it before the handler runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    GrpcV4317,
    HttpProtobufV4318,
    HttpJsonV4318,
}

/// Per-protocol span arrival counts (§7) -- "412 spans from checkout over HTTP/protobuf" is the
/// positive-signal example the Receiver panel leads with.
#[derive(Debug, Default, Clone, Copy, serde::Serialize)]
pub struct ArrivalCounts {
    pub grpc_v4317: u64,
    pub http_protobuf_v4318: u64,
    pub http_json_v4318: u64,
}

impl ArrivalCounts {
    pub fn record(&mut self, transport: Transport, span_count: u64) {
        match transport {
            Transport::GrpcV4317 => self.grpc_v4317 += span_count,
            Transport::HttpProtobufV4318 => self.http_protobuf_v4318 += span_count,
            Transport::HttpJsonV4318 => self.http_json_v4318 += span_count,
        }
    }
}

/// The rejects half of Receiver data (§7). Lives in `src/ingest/` per the doc comment on
/// `Counters` — envelope rejects are an ingest-layer concern, span-level counters are the store's.
#[derive(Debug, Default)]
pub struct RejectLog {
    ring: VecDeque<RejectRecord>,
}

impl RejectLog {
    pub fn push(&mut self, reason: String, at_unix_nano: u64) {
        if self.ring.len() == REJECT_RING_CAPACITY {
            self.ring.pop_front();
        }
        self.ring.push_back(RejectRecord { reason, at_unix_nano });
    }

    pub fn recent(&self) -> impl Iterator<Item = &RejectRecord> {
        self.ring.iter()
    }

    pub fn len(&self) -> usize {
        self.ring.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }
}

/// The full ingest-layer half of Receiver data (§7): rejects plus arrival counts. Store-level
/// counters (`duplicate_span`, eviction, ...) live in `Store::counters` -- see the doc comment on
/// `Counters` for why the split.
#[derive(Debug, Default)]
pub struct ReceiverState {
    pub rejects: RejectLog,
    pub arrivals: ArrivalCounts,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn push_records_in_order() {
        let mut log = RejectLog::default();
        log.push("EmptyPayload".to_string(), 1);
        log.push("DecodeError".to_string(), 2);

        let recorded: Vec<_> = log.recent().map(|r| r.reason.clone()).collect();
        assert_eq!(recorded, vec!["EmptyPayload", "DecodeError"]);
    }

    #[test]
    fn wraps_oldest_first_at_capacity() {
        let mut log = RejectLog::default();
        for i in 0..REJECT_RING_CAPACITY + 3 {
            log.push(format!("reject-{i}"), i as u64);
        }

        assert_eq!(log.len(), REJECT_RING_CAPACITY);
        let first = log.recent().next().unwrap();
        // the oldest 3 were pushed out; the ring now starts at reject-3
        assert_eq!(first.reason, "reject-3");
    }

    #[test]
    fn arrival_counts_accumulate_per_transport_independently() {
        let mut counts = ArrivalCounts::default();
        counts.record(Transport::GrpcV4317, 5);
        counts.record(Transport::HttpProtobufV4318, 2);
        counts.record(Transport::GrpcV4317, 3);

        assert_eq!(counts.grpc_v4317, 8);
        assert_eq!(counts.http_protobuf_v4318, 2);
        assert_eq!(counts.http_json_v4318, 0);
    }
}
