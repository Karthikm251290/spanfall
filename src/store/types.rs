#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TraceId(pub [u8; 16]);

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
}
