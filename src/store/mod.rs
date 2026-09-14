pub mod interner;
pub mod trace;
pub mod types;
pub mod writer;

use rustc_hash::FxHashMap;

use crate::receiver::Counters;
use interner::Interner;
use trace::{InsertOutcome, NewSpan, Trace};
use types::TraceId;

// ponytail: no number is pinned in spec 01 §1 for the global interner cap ("a hard cap" is
// stated but not sized), unlike --max-memory (STOP #5, pinned to 1 GiB). Picking a default here
// rather than stopping, since it's an internal implementation constant, not a product-facing
// decision — flagged for the owner to override if wrong. Bump if real traces blow past it.
const GLOBAL_INTERNER_CAP: usize = 100_000;

// Rough, deliberately approximate accounting (§1's organising insight: don't optimise the wrong
// thing). Fixed per-span overhead for the parallel-array fields plus attribute entries.
const APPROX_SPAN_FIXED_BYTES: usize = 64;
const APPROX_ATTR_FIXED_BYTES: usize = 24;

pub struct Store {
    traces: FxHashMap<TraceId, Trace>,
    key_interner: Interner,
    service_interner: Interner,
    max_memory_bytes: usize,
    // last known generation for a trace_id that was evicted, so a late-arriving span resurrects
    // it at generation+1 instead of looking like a fresh trace (§1, "Generation counter").
    evicted_generations: FxHashMap<TraceId, u32>,
    pub counters: Counters,
}

pub struct StoreInsertOutcome {
    pub local_idx: usize,
    pub is_duplicate: bool,
    pub resurrected: bool,
}

impl Store {
    pub fn new(max_memory_bytes: usize) -> Self {
        Self {
            traces: FxHashMap::default(),
            key_interner: Interner::new(GLOBAL_INTERNER_CAP),
            service_interner: Interner::new(GLOBAL_INTERNER_CAP),
            max_memory_bytes,
            evicted_generations: FxHashMap::default(),
            counters: Counters::default(),
        }
    }

    pub fn trace_count(&self) -> usize {
        self.traces.len()
    }

    pub fn trace(&self, trace_id: TraceId) -> Option<&Trace> {
        self.traces.get(&trace_id)
    }

    pub fn intern_service_name(&mut self, name: &str) -> Option<u32> {
        self.service_interner.intern(name)
    }

    pub fn insert_span(
        &mut self,
        trace_id: TraceId,
        span: NewSpan,
        now_unix_nano: u64,
    ) -> StoreInsertOutcome {
        let resurrected = !self.traces.contains_key(&trace_id) && self.evicted_generations.contains_key(&trace_id);

        let trace = self.traces.entry(trace_id).or_insert_with(|| {
            let generation = self
                .evicted_generations
                .remove(&trace_id)
                .map(|g| g + 1)
                .unwrap_or(0);
            let mut t = Trace::new(trace_id, now_unix_nano);
            t.generation = generation;
            t
        });

        let InsertOutcome { local_idx, is_duplicate } =
            trace.insert(span, &mut self.key_interner, now_unix_nano);

        if is_duplicate {
            self.counters.duplicate_span += 1;
        }
        if resurrected {
            self.counters.late_span_after_eviction += 1;
        }
        self.counters.attribute_key_cap_hit = self.key_interner.cap_hit_count();

        self.evict_over_budget();

        StoreInsertOutcome { local_idx, is_duplicate, resurrected }
    }

    fn approx_bytes(trace: &Trace) -> usize {
        let mut total = 0usize;
        for idx in 0..trace.span_count() {
            total += APPROX_SPAN_FIXED_BYTES;
            total += trace.name(idx).len();
            total += trace.attributes(idx).len() * APPROX_ATTR_FIXED_BYTES;
        }
        total
    }

    fn total_approx_bytes(&self) -> usize {
        self.traces.values().map(Self::approx_bytes).sum()
    }

    /// Evicts the trace with the oldest `last_activity` until under budget. A trace still
    /// receiving spans has a recent `last_activity` and so is never the oldest — no separate
    /// "in use" check needed (§5).
    fn evict_over_budget(&mut self) {
        while self.total_approx_bytes() > self.max_memory_bytes && self.traces.len() > 1 {
            let oldest = self
                .traces
                .iter()
                .min_by_key(|(_, t)| t.last_activity_unix_nano)
                .map(|(id, _)| *id);

            let Some(id) = oldest else { break };
            if let Some(t) = self.traces.remove(&id) {
                self.evicted_generations.insert(id, t.generation);
                self.counters.evicted_traces += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trace::NewSpan;
    use types::SpanId;

    fn tid(b: u8) -> TraceId {
        TraceId([b; 16])
    }

    fn span(id: u8, name: &str) -> NewSpan {
        NewSpan {
            span_id: SpanId([id; 8]),
            parent_span_id: None,
            name: name.to_string(),
            start_time_unix_nano: 0,
            end_time_unix_nano: 0,
            status_message: String::new(),
            unknown_service: false,
            attributes: Vec::new(),
        }
    }

    #[test]
    fn eviction_picks_oldest_last_activity_not_oldest_created() {
        // one span's approx footprint is APPROX_SPAN_FIXED_BYTES (64) + name len (1) = 65.
        // cap for ~2 traces, not 3.
        let mut store = Store::new(140);

        store.insert_span(tid(1), span(1, "a"), 0); // A created at t=0
        store.insert_span(tid(2), span(2, "b"), 1); // B created at t=1, never touched again
        // touch A again (dedupe overwrite, same span id) so its last_activity moves to t=5,
        // well after B's — even though A was created first.
        store.insert_span(tid(1), span(1, "a"), 5);

        assert_eq!(store.trace_count(), 2);

        // adding a third trace pushes over budget; the oldest *last_activity* (B, at t=1) must
        // be evicted, not A (which was created first but touched more recently).
        store.insert_span(tid(3), span(3, "c"), 10);

        assert!(store.trace(tid(1)).is_some(), "A should survive: touched most recently");
        assert!(store.trace(tid(2)).is_none(), "B should be evicted: oldest last_activity");
        assert!(store.trace(tid(3)).is_some());
        assert_eq!(store.counters.evicted_traces, 1);
    }

    #[test]
    fn late_span_after_eviction_resurrects_the_trace_and_bumps_generation() {
        let mut store = Store::new(80); // room for ~1 trace only

        store.insert_span(tid(1), span(1, "a"), 0);
        store.insert_span(tid(2), span(2, "b"), 1); // evicts trace 1

        assert!(store.trace(tid(1)).is_none());
        assert_eq!(store.counters.evicted_traces, 1);

        let outcome = store.insert_span(tid(1), span(3, "c"), 2);

        assert!(outcome.resurrected);
        assert_eq!(store.counters.late_span_after_eviction, 1);
        assert_eq!(store.trace(tid(1)).unwrap().generation, 1);
    }

    #[test]
    fn duplicate_span_increments_the_store_level_counter() {
        let mut store = Store::new(10_000);
        store.insert_span(tid(1), span(1, "a"), 0);
        let outcome = store.insert_span(tid(1), span(1, "a-renamed"), 1);

        assert!(outcome.is_duplicate);
        assert_eq!(store.counters.duplicate_span, 1);
    }
}
