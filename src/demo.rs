//! `--demo` (spec 07 T15): a curated, deterministic fake trace set seeded straight into the
//! `Store` before the listeners start -- an adoption lever, a README GIF source, and (per the
//! spec) "something for the linter to demonstrate." Demo does not need to be sequenced after
//! spec 03's rule set (that constraint was explicitly dropped, see 07-cli-demo-docs.md T15) --
//! each trace below is curated to trip one of the five rules spec 03 documents, so the moment
//! `lint --all` exists it reads real findings against this data on purpose, not by accident.
//!
//! Deterministic on purpose: no wall clock, no randomness. A README GIF and a lint fixture both
//! break if span ids or timestamps shift between runs.

use crate::store::trace::NewSpan;
use crate::store::types::{AttrValue, SpanId, TraceId};
use crate::store::Store;

fn span(
    span_id: [u8; 8],
    parent: Option<[u8; 8]>,
    name: &str,
    start: u64,
    end: u64,
    service: Option<&str>,
) -> NewSpan {
    NewSpan {
        span_id: SpanId(span_id),
        parent_span_id: parent.map(SpanId),
        name: name.to_string(),
        start_time_unix_nano: start,
        end_time_unix_nano: end,
        status_message: String::new(),
        status_code: 0,
        unknown_service: service.is_none(),
        service_name: service.map(str::to_string),
        attributes: Vec::new(),
    }
}

/// Seeds the store with the curated demo trace set. Call once, before the listeners start --
/// `evict_over_budget` runs once at the end, matching the writer's own batch-insert pattern.
pub fn seed(store: &mut Store) {
    let mut now = 0u64;
    let mut insert = |trace_id: TraceId, s: NewSpan| {
        store.insert_span_no_evict(trace_id, s, now);
        now += 1;
    };

    // A clean, well-propagated multi-service trace -- the contrast case, no findings.
    let checkout = TraceId(*b"checkout-flow-01");
    insert(checkout, span(*b"root-001", None, "POST /checkout", 0, 300_000_000, Some("checkout")));
    insert(checkout, span(*b"charge01", Some(*b"root-001"), "charge-card", 10_000_000, 180_000_000, Some("payments")));
    insert(
        checkout,
        span(*b"reserve1", Some(*b"root-001"), "reserve-inventory", 10_000_000, 90_000_000, Some("inventory")),
    );

    // Rule 2 -- missing service.name.
    let anon = TraceId(*b"missing-svc-name");
    insert(anon, span(*b"anon0001", None, "GET /health", 0, 5_000_000, None));

    // Rule 3 -- broken context propagation: 2+ services, one span's parent never arrives.
    let broken = TraceId(*b"broken-propagatn");
    insert(broken, span(*b"gateway1", None, "gateway", 0, 200_000_000, Some("gateway")));
    insert(
        broken,
        span(*b"authchld", Some(*b"neverprt"), "validate-token", 5_000_000, 40_000_000, Some("auth")),
    );

    // Rule 4 -- attribute type conflict: same key, two different value types.
    let drift = TraceId(*b"attribute-drift1");
    let mut a = span(*b"drift001", None, "GET /orders", 0, 50_000_000, Some("orders"));
    a.attributes.push(("http.status_code".to_string(), AttrValue::Int(200)));
    insert(drift, a);
    let mut b = span(*b"drift002", None, "GET /invoice", 0, 60_000_000, Some("billing"));
    b.attributes.push(("http.status_code".to_string(), AttrValue::Str("200".to_string())));
    insert(drift, b);

    // Rule 5 -- long-duration leaf: no children, > 1s, > 50% of its parent's duration.
    let slow = TraceId(*b"slow-leaf-span01");
    insert(slow, span(*b"slowroot", None, "batch-export", 0, 3_000_000_000, Some("reporting")));
    insert(slow, span(*b"slowleaf", Some(*b"slowroot"), "render-pdf", 0, 2_000_000_000, Some("reporting")));

    // Rule 1 -- high-cardinality span names (>= 200 distinct). One service, 200 uniquely named
    // leaf spans under a normal root -- a realistic shape for e.g. a search service that puts a
    // raw query or id into the span name instead of a fixed operation label.
    let search = TraceId(*b"high-cardinality");
    insert(search, span(*b"searchrt", None, "search-request", 0, 500_000_000, Some("search")));
    for i in 0..200u32 {
        let span_id = {
            let mut id = [0u8; 8];
            id[..4].copy_from_slice(b"srch");
            id[4..].copy_from_slice(&i.to_be_bytes());
            id
        };
        insert(search, span(span_id, Some(*b"searchrt"), &format!("query:{i}"), 1_000_000, 2_000_000, Some("search")));
    }

    store.evict_over_budget();
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn seeded() -> Store {
        let mut store = Store::new(usize::MAX);
        seed(&mut store);
        store
    }

    #[test]
    fn seeds_a_missing_service_name_span() {
        let store = seeded();
        let found = store.traces().any(|(_, t)| (0..t.span_count()).any(|i| t.unknown_service(i)));
        assert!(found, "expected at least one unknown_service span");
    }

    #[test]
    fn seeds_an_orphan_span_in_a_multi_service_trace() {
        let store = seeded();
        let broken = TraceId(*b"broken-propagatn");
        let trace = store.trace(broken).expect("broken-propagation trace seeded");

        let mut services = HashSet::new();
        let mut has_orphan = false;
        for i in 0..trace.span_count() {
            has_orphan |= trace.is_orphan(i);
            if let Some(id) = trace.service_name_id(i) {
                services.insert(store.service_name(id).to_string());
            }
        }
        assert!(has_orphan, "expected an orphan span");
        assert!(services.len() >= 2, "expected 2+ services, got {services:?}");
    }

    #[test]
    fn seeds_the_same_attribute_key_with_two_different_value_types() {
        let store = seeded();
        let drift = TraceId(*b"attribute-drift1");
        let trace = store.trace(drift).expect("attribute-drift trace seeded");

        let mut kinds: HashSet<&'static str> = HashSet::new();
        for i in 0..trace.span_count() {
            for (&key_id, value) in trace.attribute_keys(i).iter().zip(trace.attributes(i).iter()) {
                if store.attribute_key(key_id) == "http.status_code" {
                    kinds.insert(match value {
                        AttrValue::Str(_) => "str",
                        AttrValue::Int(_) => "int",
                        AttrValue::Double(_) => "double",
                        AttrValue::Bool(_) => "bool",
                    });
                }
            }
        }
        assert_eq!(kinds.len(), 2, "expected http.status_code seeded as two distinct types, got {kinds:?}");
    }

    #[test]
    fn seeds_a_long_leaf_span_over_half_its_parents_duration() {
        let store = seeded();
        let slow = TraceId(*b"slow-leaf-span01");
        let trace = store.trace(slow).expect("slow-leaf trace seeded");

        let parent_duration = trace.duration_nanos().expect("trace has a duration");
        let mut found = false;
        for i in 0..trace.span_count() {
            let Some(_parent) = trace.parent_idx(i) else { continue };
            let duration = trace.end_time_unix_nano(i) - trace.start_time_unix_nano(i);
            if duration > 1_000_000_000 && duration * 2 > parent_duration {
                found = true;
            }
        }
        assert!(found, "expected a leaf span over 1s and over half its parent's duration");
    }

    #[test]
    fn seeds_at_least_two_hundred_distinct_span_names() {
        let store = seeded();
        let mut names = HashSet::new();
        for (_, trace) in store.traces() {
            for i in 0..trace.span_count() {
                names.insert(trace.name(i).to_string());
            }
        }
        assert!(names.len() >= 200, "expected >= 200 distinct span names, got {}", names.len());
    }

    #[test]
    fn seeding_is_deterministic_across_runs() {
        let a = seeded();
        let b = seeded();
        assert_eq!(a.trace_count(), b.trace_count());

        let checkout = TraceId(*b"checkout-flow-01");
        let ta = a.trace(checkout).expect("seeded");
        let tb = b.trace(checkout).expect("seeded");
        assert_eq!(ta.span_count(), tb.span_count());
        for i in 0..ta.span_count() {
            assert_eq!(ta.name(i), tb.name(i));
            assert_eq!(ta.start_time_unix_nano(i), tb.start_time_unix_nano(i));
        }
    }
}
