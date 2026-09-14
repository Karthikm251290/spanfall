use std::collections::VecDeque;

// ponytail: no size pinned by spec 01 §7 for the rejects ring, only that it "wraps oldest-first
// at capacity" and that wraparound is a tested behaviour. 100 is a reasonable Receiver-panel
// window; bump if a real session needs more history.
const REJECT_RING_CAPACITY: usize = 100;

/// One envelope-level reject (§2's `EmptyPayload` / `DecodeError`), recorded for the Receiver
/// panel (§7) — "must be reported, not shown as a silent blank screen".
#[derive(Debug, Clone)]
pub struct RejectRecord {
    pub reason: String,
    pub at_unix_nano: u64,
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
}
