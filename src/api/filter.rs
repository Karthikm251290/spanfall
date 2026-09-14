use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use parking_lot::RwLock;
use serde::Serialize;

use crate::store::trace::Trace;
use crate::store::types::{AttrValue, TraceId};
use crate::store::Store;

#[derive(Serialize)]
struct FilterResponse {
    // acceptance #4: indices only, never span objects.
    indices: Vec<usize>,
    dropped_terms: Vec<String>,
}

/// `GET /api/traces/:id/filter?q=...` (spec 01 §6). Grammar:
/// `query := term (WS term)*`, `term := KEY OP VALUE | bareword`. A malformed term (unknown
/// operator, unterminated quote) is dropped and reported back rather than failing the query.
pub async fn get_filter(
    State(store): State<Arc<RwLock<Store>>>,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let Some(trace_id) = TraceId::from_hex(&id) else {
        return (StatusCode::BAD_REQUEST, "trace id must be 32 hex characters").into_response();
    };

    let guard = store.read();
    let Some(trace) = guard.trace(trace_id) else {
        drop(guard);
        return StatusCode::NOT_FOUND.into_response();
    };

    let query = params.get("q").map(String::as_str).unwrap_or("");
    let (terms, dropped_terms) = parse_query(query, &guard);

    let indices: Vec<usize> =
        (0..trace.span_count()).filter(|&idx| terms.iter().all(|t| t.matches(trace, &guard, idx))).collect();
    drop(guard);

    Json(FilterResponse { indices, dropped_terms }).into_response()
}

#[derive(Clone, Copy, PartialEq)]
enum Op {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
}

// longest-prefix-first, so ">=" is tried before "=".
const OPS: [(&str, Op); 6] =
    [(">=", Op::Gte), ("<=", Op::Lte), ("!=", Op::Ne), ("=", Op::Eq), (">", Op::Gt), ("<", Op::Lt)];

enum Term {
    Service(Op, String),
    Status(Op, i32),
    Duration(Op, u64),
    // `None` id = key never interned by any span -- matches nothing, not a dropped term.
    Attr(Op, Option<u32>, String),
    Substring(String),
}

impl Term {
    fn matches(&self, trace: &Trace, store: &Store, idx: usize) -> bool {
        match self {
            Term::Service(op, want) => {
                let got = trace.service_name_id(idx).map(|id| store.service_name(id));
                match (op, got) {
                    (Op::Eq, Some(name)) => name == want,
                    (Op::Ne, Some(name)) => name != want,
                    (Op::Ne, None) => true,
                    _ => false,
                }
            }
            Term::Status(op, want) => match op {
                Op::Eq => trace.status_code(idx) == *want,
                Op::Ne => trace.status_code(idx) != *want,
                _ => false,
            },
            Term::Duration(op, want) => {
                match trace.end_time_unix_nano(idx).checked_sub(trace.start_time_unix_nano(idx)) {
                    Some(d) => cmp(*op, d, *want),
                    None => false,
                }
            }
            Term::Attr(op, Some(key_id), raw) => trace
                .attribute_keys(idx)
                .iter()
                .zip(trace.attributes(idx).iter())
                .filter(|(k, _)| *k == key_id)
                .any(|(_, v)| attr_matches(*op, v, raw)),
            Term::Attr(_, None, _) => false,
            Term::Substring(needle) => {
                trace.name(idx).contains(needle.as_str())
                    || trace.attributes(idx).iter().any(|v| attr_contains(v, needle))
            }
        }
    }
}

fn cmp<T: PartialOrd + PartialEq>(op: Op, got: T, want: T) -> bool {
    match op {
        Op::Eq => got == want,
        Op::Ne => got != want,
        Op::Gt => got > want,
        Op::Gte => got >= want,
        Op::Lt => got < want,
        Op::Lte => got <= want,
    }
}

fn attr_matches(op: Op, attr: &AttrValue, raw: &str) -> bool {
    match op {
        Op::Eq | Op::Ne => {
            let eq = match attr {
                AttrValue::Str(s) => s == raw,
                AttrValue::Int(i) => raw.parse::<i64>().is_ok_and(|v| v == *i),
                AttrValue::Double(d) => raw.parse::<f64>().is_ok_and(|v| v == *d),
                AttrValue::Bool(b) => matches!((b, raw), (true, "true") | (false, "false")),
            };
            if op == Op::Eq { eq } else { !eq }
        }
        Op::Gt | Op::Gte | Op::Lt | Op::Lte => {
            let attr_num = match attr {
                AttrValue::Int(i) => *i as f64,
                AttrValue::Double(d) => *d,
                AttrValue::Str(_) | AttrValue::Bool(_) => return false,
            };
            match raw.parse::<f64>() {
                Ok(want) => cmp(op, attr_num, want),
                Err(_) => false,
            }
        }
    }
}

fn attr_contains(attr: &AttrValue, needle: &str) -> bool {
    match attr {
        AttrValue::Str(s) => s.contains(needle),
        AttrValue::Int(i) => i.to_string().contains(needle),
        AttrValue::Double(d) => d.to_string().contains(needle),
        AttrValue::Bool(b) => b.to_string().contains(needle),
    }
}

fn parse_duration(raw: &str) -> Option<u64> {
    let (num, unit_nanos) = if let Some(n) = raw.strip_suffix("ms") {
        (n, 1_000_000)
    } else if let Some(n) = raw.strip_suffix("us") {
        (n, 1_000)
    } else if let Some(n) = raw.strip_suffix("ns") {
        (n, 1)
    } else if let Some(n) = raw.strip_suffix('s') {
        (n, 1_000_000_000)
    } else {
        (raw, 1)
    };
    num.parse::<u64>().ok()?.checked_mul(unit_nanos)
}

fn is_key_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_key_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.'
}

/// Splits on whitespace, quote-aware: a `"..."` span (backslash-quote is the only escape) is
/// never split on the whitespace it contains. Returns each raw token, whether its opening quote
/// (if any) was never closed (unterminated -- malformed, dropped), and whether the token's very
/// first character was a quote (a standalone quoted bareword, e.g. `"db query"` -- always a
/// literal substring term, never re-parsed as `KEY OP VALUE` even if its content looks like one).
fn tokenize(q: &str) -> Vec<(String, bool, bool)> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut starts_quoted = false;
    let mut chars = q.chars().peekable();

    while let Some(c) = chars.next() {
        if in_quotes && c == '\\' && chars.peek() == Some(&'"') {
            cur.push('"');
            chars.next();
        } else if c == '"' {
            if cur.is_empty() && !in_quotes {
                starts_quoted = true;
            }
            in_quotes = !in_quotes;
        } else if c.is_whitespace() && !in_quotes {
            if !cur.is_empty() {
                tokens.push((std::mem::take(&mut cur), false, starts_quoted));
                starts_quoted = false;
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        tokens.push((cur, in_quotes, starts_quoted));
    }
    tokens
}

/// KEY OP VALUE, or a bareword substring term with no key/operator at all (§6).
fn parse_term<'a>(text: &'a str, starts_quoted: bool, store: &Store) -> Result<Term, &'a str> {
    if starts_quoted {
        return Ok(Term::Substring(text.to_string()));
    }

    let key_len = text.chars().take_while(|&c| is_key_char(c)).count();
    let key_len = if text.chars().next().is_some_and(is_key_start) { key_len } else { 0 };
    if key_len == 0 {
        return Ok(Term::Substring(text.to_string()));
    }

    let key = &text[..key_len];
    let rest = &text[key_len..];
    if rest.is_empty() {
        return Ok(Term::Substring(text.to_string()));
    }

    let Some(&(prefix, op)) = OPS.iter().find(|(prefix, _)| rest.starts_with(prefix)) else {
        return Err(text); // unknown operator
    };
    let value = &rest[prefix.len()..];

    match key {
        "service" if matches!(op, Op::Eq | Op::Ne) => Ok(Term::Service(op, value.to_string())),
        "status" if matches!(op, Op::Eq | Op::Ne) => match value {
            "unset" => Ok(Term::Status(op, 0)),
            "ok" => Ok(Term::Status(op, 1)),
            "error" => Ok(Term::Status(op, 2)),
            _ => Err(text),
        },
        "duration" => match parse_duration(value) {
            Some(nanos) => Ok(Term::Duration(op, nanos)),
            None => Err(text),
        },
        "service" | "status" => Err(text), // ordering ops make no sense for these two keys
        _ => Ok(Term::Attr(op, store.attribute_key_id(key), value.to_string())),
    }
}

fn parse_query(q: &str, store: &Store) -> (Vec<Term>, Vec<String>) {
    let mut terms = Vec::new();
    let mut dropped = Vec::new();

    for (text, unterminated, starts_quoted) in tokenize(q) {
        if unterminated {
            dropped.push(text);
            continue;
        }
        match parse_term(&text, starts_quoted, store) {
            Ok(term) => terms.push(term),
            Err(raw) => dropped.push(raw.to_string()),
        }
    }
    (terms, dropped)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    use super::*;
    use crate::store::trace::NewSpan;
    use crate::store::types::SpanId;

    fn store_with_two_spans() -> Arc<RwLock<Store>> {
        let mut store = Store::new(10_000_000);
        store.insert_span(
            TraceId([1; 16]),
            NewSpan {
                span_id: SpanId([1; 8]),
                parent_span_id: None,
                name: "checkout root".to_string(),
                start_time_unix_nano: 0,
                end_time_unix_nano: 50_000_000, // 50ms
                status_message: String::new(),
                status_code: 1, // Ok
                unknown_service: false,
                service_name: Some("checkout".to_string()),
                attributes: vec![("http.status_code".to_string(), AttrValue::Int(200))],
            },
            0,
        );
        store.insert_span(
            TraceId([1; 16]),
            NewSpan {
                span_id: SpanId([2; 8]),
                parent_span_id: None,
                name: "db query".to_string(),
                start_time_unix_nano: 0,
                end_time_unix_nano: 200_000_000, // 200ms
                status_message: String::new(),
                status_code: 2, // Error
                unknown_service: false,
                service_name: Some("db".to_string()),
                attributes: vec![("http.status_code".to_string(), AttrValue::Int(500))],
            },
            0,
        );
        Arc::new(RwLock::new(store))
    }

    async fn filter(store: Arc<RwLock<Store>>, q: &str) -> serde_json::Value {
        let app = super::super::router(store);
        let hex = TraceId([1; 16]).to_hex();
        let uri = format!("/api/traces/{hex}/filter?q={}", urlencoding_lite(q));
        let response = app.oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    // ponytail: axum's Query<HashMap<_,_>> percent-decodes for us; tests only need spaces and
    // quotes escaped, not a full encoder.
    fn urlencoding_lite(s: &str) -> String {
        s.replace(' ', "%20").replace('"', "%22").replace('>', "%3E").replace('<', "%3C")
    }

    #[tokio::test]
    async fn status_error_matches_only_the_error_span() {
        let body = filter(store_with_two_spans(), "status=error").await;
        assert_eq!(body["indices"], serde_json::json!([1]));
        assert_eq!(body["dropped_terms"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn duration_is_per_span_not_the_trace_aggregate() {
        // trace-wide duration would be 200ms for every span if this reached for the wrong
        // aggregate; only the 200ms span should pass a >100ms filter.
        let body = filter(store_with_two_spans(), "duration>100ms").await;
        assert_eq!(body["indices"], serde_json::json!([1]));
    }

    #[tokio::test]
    async fn numeric_attribute_ordering_works() {
        let body = filter(store_with_two_spans(), "http.status_code>=500").await;
        assert_eq!(body["indices"], serde_json::json!([1]));
    }

    #[tokio::test]
    async fn implicit_and_across_multiple_terms() {
        let body = filter(store_with_two_spans(), "service=checkout status=ok").await;
        assert_eq!(body["indices"], serde_json::json!([0]));
    }

    #[tokio::test]
    async fn bareword_matches_span_name_as_substring() {
        let body = filter(store_with_two_spans(), "checkout").await;
        assert_eq!(body["indices"], serde_json::json!([0]));
    }

    #[tokio::test]
    async fn quoted_bareword_with_a_space_matches_as_one_substring_term() {
        let body = filter(store_with_two_spans(), "\"db query\"").await;
        assert_eq!(body["indices"], serde_json::json!([1]));
    }

    #[tokio::test]
    async fn unknown_operator_is_dropped_but_the_rest_of_the_query_still_applies() {
        let body = filter(store_with_two_spans(), "service~checkout status=ok").await;
        assert_eq!(body["indices"], serde_json::json!([0]));
        assert_eq!(body["dropped_terms"], serde_json::json!(["service~checkout"]));
    }

    #[tokio::test]
    async fn unresolvable_key_matches_nothing_and_is_not_reported_as_dropped() {
        let body = filter(store_with_two_spans(), "nonexistent.key=x").await;
        assert_eq!(body["indices"], serde_json::json!([]));
        assert_eq!(body["dropped_terms"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn missing_query_matches_every_span() {
        let body = filter(store_with_two_spans(), "").await;
        assert_eq!(body["indices"], serde_json::json!([0, 1]));
    }

    #[tokio::test]
    async fn filter_returns_indices_never_span_objects() {
        let body = filter(store_with_two_spans(), "service=checkout").await;
        let indices = body["indices"].as_array().unwrap();
        assert!(indices.iter().all(|v| v.is_number()));
        assert!(body.get("spans").is_none());
    }

    #[tokio::test]
    async fn get_filter_returns_404_for_an_unknown_trace() {
        let app = super::super::router(store_with_two_spans());
        let hex = TraceId([9; 16]).to_hex();

        let response = app
            .oneshot(Request::builder().uri(format!("/api/traces/{hex}/filter?q=x")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
