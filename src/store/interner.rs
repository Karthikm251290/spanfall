use rustc_hash::FxHashMap;

/// Global interner for attribute keys and service names.
///
/// Hard-capped by design (§1, `01-m0-store-ingest.md`): once `cap` distinct strings are
/// interned, further new strings are refused rather than growing unbounded. The caller counts
/// the refusal (`attribute_key_cap_hit`) instead of the interner silently ballooning on
/// adversarial input.
pub struct Interner {
    map: FxHashMap<Box<str>, u32>,
    values: Vec<Box<str>>,
    cap: usize,
    cap_hit_count: u64,
}

impl Interner {
    pub fn new(cap: usize) -> Self {
        Self {
            map: FxHashMap::default(),
            values: Vec::new(),
            cap,
            cap_hit_count: 0,
        }
    }

    /// Returns the id for `s`, interning it if new and under cap. Returns `None` if `s` is new
    /// and the cap is already full — the caller decides what to do with a refused key (§1: drop
    /// that one attribute, keep the rest of the span).
    pub fn intern(&mut self, s: &str) -> Option<u32> {
        if let Some(&id) = self.map.get(s) {
            return Some(id);
        }
        if self.values.len() >= self.cap {
            self.cap_hit_count += 1;
            return None;
        }
        let id = self.values.len() as u32;
        let boxed: Box<str> = s.into();
        self.values.push(boxed.clone());
        self.map.insert(boxed, id);
        Some(id)
    }

    pub fn get(&self, id: u32) -> &str {
        &self.values[id as usize]
    }

    pub fn cap_hit_count(&self) -> u64 {
        self.cap_hit_count
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interning_the_same_string_twice_returns_the_same_id() {
        let mut i = Interner::new(10);
        let a = i.intern("service.name").unwrap();
        let b = i.intern("service.name").unwrap();
        assert_eq!(a, b);
        assert_eq!(i.len(), 1);
    }

    #[test]
    fn get_returns_the_original_string() {
        let mut i = Interner::new(10);
        let id = i.intern("http.method").unwrap();
        assert_eq!(i.get(id), "http.method");
    }

    #[test]
    fn cap_reached_refuses_new_strings_but_keeps_serving_existing_ones() {
        let mut i = Interner::new(2);
        let a = i.intern("a").unwrap();
        let b = i.intern("b").unwrap();

        // cap is full: a brand new string is refused, not evicted-and-replaced
        assert_eq!(i.intern("c"), None);
        assert_eq!(i.cap_hit_count(), 1);

        // already-interned strings keep resolving fine
        assert_eq!(i.intern("a"), Some(a));
        assert_eq!(i.get(b), "b");
        assert_eq!(i.len(), 2);
    }

    #[test]
    fn repeated_refusals_keep_counting() {
        let mut i = Interner::new(1);
        i.intern("a").unwrap();
        assert!(i.intern("x").is_none());
        assert!(i.intern("y").is_none());
        assert_eq!(i.cap_hit_count(), 2);
    }
}
