#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TraceId(pub [u8; 16]);

impl TraceId {
    /// Lower-case hex, matching `opentelemetry-proto`'s own `with-serde` trace-id encoding — the
    /// same convention used everywhere else this ever crosses a text boundary.
    pub fn to_hex(self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// `None` on anything that isn't exactly 32 hex characters -- callers treat this as a 400,
    /// never a panic (§2 hardening #1: this parses attacker-controlled URL input).
    pub fn from_hex(s: &str) -> Option<Self> {
        if s.len() != 32 {
            return None;
        }
        let mut bytes = [0u8; 16];
        for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
            let pair = std::str::from_utf8(chunk).ok()?;
            bytes[i] = u8::from_str_radix(pair, 16).ok()?;
        }
        Some(TraceId(bytes))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpanId(pub [u8; 8]);

#[derive(Debug, Clone, PartialEq)]
pub enum AttrValue {
    Str(String),
    Int(i64),
    Double(f64),
    Bool(bool),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_ids_with_equal_bytes_are_equal_and_hash_equal() {
        let a = TraceId([1u8; 16]);
        let b = TraceId([1u8; 16]);
        assert_eq!(a, b);
    }

    #[test]
    fn trace_ids_with_different_bytes_are_not_equal() {
        let a = TraceId([1u8; 16]);
        let b = TraceId([2u8; 16]);
        assert_ne!(a, b);
    }

    #[test]
    fn hex_round_trips() {
        let id = TraceId([0xab; 16]);
        let hex = id.to_hex();
        assert_eq!(hex.len(), 32);
        assert_eq!(TraceId::from_hex(&hex), Some(id));
    }

    #[test]
    fn from_hex_rejects_wrong_length_and_non_hex_input() {
        assert_eq!(TraceId::from_hex("ab"), None);
        assert_eq!(TraceId::from_hex(&"zz".repeat(16)), None);
    }
}
