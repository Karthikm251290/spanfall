/// Counters produced as a byproduct of store mutations (§7, `01-m0-store-ingest.md`).
///
/// This is the store-layer slice only: per-endpoint/protocol arrival counts and the rejects ring
/// buffer are ingest-layer concerns and land with `src/ingest/`.
#[derive(Debug, Default, Clone, Copy)]
pub struct Counters {
    pub duplicate_span: u64,
    pub attribute_key_cap_hit: u64,
    pub late_span_after_eviction: u64,
    pub evicted_traces: u64,
    /// Per-span decode failures within an otherwise-parseable envelope (§2, PartialBatch) --
    /// the span is dropped, the rest of the batch is kept.
    pub malformed_span: u64,
}
