use rustc_hash::FxHashMap;

use super::interner::Interner;
use super::types::{AttrValue, SpanId, TraceId};

#[derive(Debug)]
pub struct NewSpan {
    pub span_id: SpanId,
    pub parent_span_id: Option<SpanId>,
    pub name: String,
    pub start_time_unix_nano: u64,
    pub end_time_unix_nano: u64,
    pub status_message: String,
    pub unknown_service: bool,
    pub attributes: Vec<(String, AttrValue)>,
}

pub struct InsertOutcome {
    pub local_idx: usize,
    pub is_duplicate: bool,
}

/// One trace's spans, struct-of-arrays, indexed by local span index `0..n`.
///
/// Attributes are CSR-ragged: `attr_offsets[i]..attr_offsets[i+1]` is span `i`'s slice into the
/// dense `attr_keys`/`attr_values` arrays. `attr_offsets.len() == n + 1` always; the sentinel is
/// pushed at construction, not on first insert (§1 — this is where the off-by-one lives).
///
/// ponytail: span names / status messages live in an owned `String` per span rather than a
/// packed per-trace byte arena. Satisfies §1's actual requirement ("freed when the trace is
/// evicted" — dropping the Trace drops the Strings) with far less code. Pack into a byte arena
/// if M1's bytes/span measurement demands it.
pub struct Trace {
    pub trace_id: TraceId,
    pub generation: u32,
    pub last_activity_unix_nano: u64,

    span_ids: Vec<SpanId>,
    names: Vec<String>,
    parent_idx: Vec<Option<usize>>,
    start_time_unix_nano: Vec<u64>,
    end_time_unix_nano: Vec<u64>,
    status_message: Vec<String>,
    orphan: Vec<bool>,
    unknown_service: Vec<bool>,

    attr_keys: Vec<u32>,
    attr_values: Vec<AttrValue>,
    attr_offsets: Vec<u32>,

    span_index: FxHashMap<SpanId, usize>,
    pending_children: FxHashMap<SpanId, Vec<usize>>,
}

impl Trace {
    pub fn new(trace_id: TraceId, now_unix_nano: u64) -> Self {
        Self {
            trace_id,
            generation: 0,
            last_activity_unix_nano: now_unix_nano,
            span_ids: Vec::new(),
            names: Vec::new(),
            parent_idx: Vec::new(),
            start_time_unix_nano: Vec::new(),
            end_time_unix_nano: Vec::new(),
            status_message: Vec::new(),
            orphan: Vec::new(),
            unknown_service: Vec::new(),
            attr_keys: Vec::new(),
            attr_values: Vec::new(),
            attr_offsets: vec![0],
            span_index: FxHashMap::default(),
            pending_children: FxHashMap::default(),
        }
    }

    pub fn span_count(&self) -> usize {
        self.span_ids.len()
    }

    pub fn bump_generation(&mut self) {
        self.generation += 1;
    }

    pub fn is_orphan(&self, idx: usize) -> bool {
        self.orphan[idx]
    }

    pub fn parent_idx(&self, idx: usize) -> Option<usize> {
        self.parent_idx[idx]
    }

    pub fn name(&self, idx: usize) -> &str {
        &self.names[idx]
    }

    pub fn span_id(&self, idx: usize) -> SpanId {
        self.span_ids[idx]
    }

    pub fn attributes(&self, idx: usize) -> &[AttrValue] {
        let start = self.attr_offsets[idx] as usize;
        let end = self.attr_offsets[idx + 1] as usize;
        &self.attr_values[start..end]
    }

    pub fn attribute_keys(&self, idx: usize) -> &[u32] {
        let start = self.attr_offsets[idx] as usize;
        let end = self.attr_offsets[idx + 1] as usize;
        &self.attr_keys[start..end]
    }

    /// Inserts or, on a duplicate `span_id`, overwrites in place (last-write-wins, §T4).
    ///
    /// Attribute keys are interned via `key_interner`; a key refused by a full interner drops
    /// that one attribute pair and keeps the rest of the span (§1 — never reject the span).
    pub fn insert(
        &mut self,
        mut span: NewSpan,
        key_interner: &mut Interner,
        now_unix_nano: u64,
    ) -> InsertOutcome {
        self.last_activity_unix_nano = now_unix_nano;

        let interned_attrs: Vec<(u32, AttrValue)> = std::mem::take(&mut span.attributes)
            .into_iter()
            .filter_map(|(k, v)| key_interner.intern(&k).map(|id| (id, v)))
            .collect();

        if let Some(&idx) = self.span_index.get(&span.span_id) {
            self.overwrite_at(idx, span, interned_attrs);
            return InsertOutcome { local_idx: idx, is_duplicate: true };
        }

        let idx = self.span_ids.len();
        self.span_ids.push(span.span_id);
        self.names.push(span.name);
        self.start_time_unix_nano.push(span.start_time_unix_nano);
        self.end_time_unix_nano.push(span.end_time_unix_nano);
        self.status_message.push(span.status_message);
        self.unknown_service.push(span.unknown_service);

        let (parent_idx, orphan) = match span.parent_span_id {
            None => (None, false),
            Some(pid) => match self.span_index.get(&pid) {
                Some(&pidx) => (Some(pidx), false),
                None => {
                    self.pending_children.entry(pid).or_default().push(idx);
                    (None, true)
                }
            },
        };
        self.parent_idx.push(parent_idx);
        self.orphan.push(orphan);

        for (k, v) in &interned_attrs {
            self.attr_keys.push(*k);
            self.attr_values.push(v.clone());
        }
        self.attr_offsets.push(self.attr_keys.len() as u32);

        self.span_index.insert(span.span_id, idx);

        // patch-in-place: this span may be the parent children were already waiting on
        if let Some(children) = self.pending_children.remove(&span.span_id) {
            for child_idx in children {
                self.parent_idx[child_idx] = Some(idx);
                self.orphan[child_idx] = false;
            }
        }

        InsertOutcome { local_idx: idx, is_duplicate: false }
    }

    fn overwrite_at(&mut self, idx: usize, span: NewSpan, interned_attrs: Vec<(u32, AttrValue)>) {
        self.names[idx] = span.name;
        self.start_time_unix_nano[idx] = span.start_time_unix_nano;
        self.end_time_unix_nano[idx] = span.end_time_unix_nano;
        self.status_message[idx] = span.status_message;
        self.unknown_service[idx] = span.unknown_service;

        let old_start = self.attr_offsets[idx] as usize;
        let old_end = self.attr_offsets[idx + 1] as usize;
        let new_len = interned_attrs.len();

        let (new_keys, new_values): (Vec<u32>, Vec<AttrValue>) = interned_attrs.into_iter().unzip();
        self.attr_keys.splice(old_start..old_end, new_keys);
        self.attr_values.splice(old_start..old_end, new_values);

        let delta = new_len as i64 - (old_end - old_start) as i64;
        for off in self.attr_offsets.iter_mut().skip(idx + 1) {
            *off = (*off as i64 + delta) as u32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sid(b: u8) -> SpanId {
        SpanId([b; 8])
    }

    fn span(id: u8, parent: Option<u8>, name: &str) -> NewSpan {
        NewSpan {
            span_id: sid(id),
            parent_span_id: parent.map(sid),
            name: name.to_string(),
            start_time_unix_nano: 100,
            end_time_unix_nano: 200,
            status_message: String::new(),
            unknown_service: false,
            attributes: Vec::new(),
        }
    }

    #[test]
    fn attr_offsets_len_is_always_span_count_plus_one() {
        let mut t = Trace::new(TraceId([0; 16]), 0);
        let mut interner = Interner::new(100);
        assert_eq!(t.attr_offsets, vec![0]);

        t.insert(span(1, None, "root"), &mut interner, 0);
        assert_eq!(t.attr_offsets.len(), 2);

        t.insert(span(2, Some(1), "child"), &mut interner, 0);
        assert_eq!(t.attr_offsets.len(), 3);
    }

    #[test]
    fn zero_attribute_span_gets_an_empty_slice() {
        let mut t = Trace::new(TraceId([0; 16]), 0);
        let mut interner = Interner::new(100);
        let out = t.insert(span(1, None, "root"), &mut interner, 0);
        assert_eq!(t.attributes(out.local_idx), &[] as &[AttrValue]);
    }

    #[test]
    fn parent_first_resolves_immediately() {
        let mut t = Trace::new(TraceId([0; 16]), 0);
        let mut interner = Interner::new(100);
        let root = t.insert(span(1, None, "root"), &mut interner, 0);
        let child = t.insert(span(2, Some(1), "child"), &mut interner, 0);

        assert_eq!(t.parent_idx(child.local_idx), Some(root.local_idx));
        assert!(!t.is_orphan(child.local_idx));
    }

    #[test]
    fn child_first_is_orphan_then_patches_when_parent_arrives() {
        let mut t = Trace::new(TraceId([0; 16]), 0);
        let mut interner = Interner::new(100);
        let child = t.insert(span(2, Some(1), "child"), &mut interner, 0);

        assert!(t.is_orphan(child.local_idx));
        assert_eq!(t.parent_idx(child.local_idx), None);

        let root = t.insert(span(1, None, "root"), &mut interner, 0);

        assert!(!t.is_orphan(child.local_idx));
        assert_eq!(t.parent_idx(child.local_idx), Some(root.local_idx));
    }

    #[test]
    fn parent_never_arriving_renders_as_a_flagged_root() {
        let mut t = Trace::new(TraceId([0; 16]), 0);
        let mut interner = Interner::new(100);
        let child = t.insert(span(2, Some(99), "child"), &mut interner, 0);

        assert!(t.is_orphan(child.local_idx));
        assert_eq!(t.parent_idx(child.local_idx), None);
        assert_eq!(t.span_count(), 1);
    }

    #[test]
    fn duplicate_span_id_overwrites_in_place_without_growing_span_count() {
        let mut t = Trace::new(TraceId([0; 16]), 0);
        let mut interner = Interner::new(100);
        let first = t.insert(span(1, None, "root"), &mut interner, 0);

        let mut updated = span(1, None, "root-renamed");
        updated.end_time_unix_nano = 999;
        let second = t.insert(updated, &mut interner, 5);

        assert_eq!(t.span_count(), 1);
        assert!(second.is_duplicate);
        assert_eq!(second.local_idx, first.local_idx);
        assert_eq!(t.name(first.local_idx), "root-renamed");
        assert_eq!(t.last_activity_unix_nano, 5);
    }

    #[test]
    fn duplicate_with_different_attribute_count_keeps_csr_offsets_consistent() {
        let mut t = Trace::new(TraceId([0; 16]), 0);
        let mut interner = Interner::new(100);

        let mut first = span(1, None, "a");
        first.attributes = vec![("k1".to_string(), AttrValue::Bool(true))];
        t.insert(first, &mut interner, 0);
        let second_span = t.insert(span(2, None, "b"), &mut interner, 0);

        // now re-send span 1 with three attributes instead of one
        let mut retry = span(1, None, "a");
        retry.attributes = vec![
            ("k1".to_string(), AttrValue::Bool(false)),
            ("k2".to_string(), AttrValue::Int(7)),
            ("k3".to_string(), AttrValue::Double(1.5)),
        ];
        t.insert(retry, &mut interner, 1);

        assert_eq!(t.attributes(0).len(), 3);
        // span 2, which comes after span 1 in the dense arrays, must still resolve correctly
        assert_eq!(t.attributes(second_span.local_idx).len(), 0);
        assert_eq!(t.attr_offsets.len(), t.span_count() + 1);
    }

    #[test]
    fn attribute_key_refused_by_a_full_interner_drops_only_that_pair() {
        let mut t = Trace::new(TraceId([0; 16]), 0);
        let mut interner = Interner::new(1);
        interner.intern("already-full").unwrap();

        let mut s = span(1, None, "root");
        s.attributes = vec![
            ("brand-new-key".to_string(), AttrValue::Str("x".to_string())),
        ];
        let out = t.insert(s, &mut interner, 0);

        assert_eq!(t.span_count(), 1);
        assert_eq!(t.attributes(out.local_idx).len(), 0);
        assert_eq!(interner.cap_hit_count(), 1);
    }
}
