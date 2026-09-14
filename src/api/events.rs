use std::convert::Infallible;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;
use tokio_stream::{Stream, StreamExt};

/// `GET /api/events` (§6) -- SSE, invalidation-only: `{"type":"changed","seq":N}`. **No span
/// data on this stream**, so the browser has one rendering path (re-fetch on change) instead of
/// separate initial-load and streaming-update code. `WatchStream` yields the current seq
/// immediately on connect, then again each time the writer bumps it (once per applied batch).
pub async fn get_events(
    State(seq): State<watch::Receiver<u64>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = WatchStream::new(seq).map(|seq| Ok(Event::default().data(format!(r#"{{"type":"changed","seq":{seq}}}"#))));
    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn connecting_yields_the_current_seq_immediately() {
        let (tx, _rx) = watch::channel(3u64);
        let mut state = super::super::test_state(crate::store::Store::new(10_000_000));
        state.events_seq = tx.subscribe();
        // close the channel before reading the body -- SSE streams never end on their own, and
        // WatchStream ends cleanly once every sender is gone, after still yielding the current
        // value it already captured on subscribe.
        drop(tx);

        let app = super::super::router(state);
        let response =
            app.oneshot(Request::builder().uri("/api/events").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(body.contains(r#"{"type":"changed","seq":3}"#), "body was: {body}");
    }

    #[tokio::test]
    async fn carries_no_span_data() {
        let (tx, _rx) = watch::channel(0u64);
        let mut state = super::super::test_state(crate::store::Store::new(10_000_000));
        state.events_seq = tx.subscribe();
        drop(tx);

        let app = super::super::router(state);
        let response =
            app.oneshot(Request::builder().uri("/api/events").body(Body::empty()).unwrap()).await.unwrap();

        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(!body.contains("span"), "SSE payload must carry no span data: {body}");
    }
}
