//! The Read API, served on the UI port (`:5317`, STOP #2), separate from the OTLP ingest ports.
//! Route contract is spec 01 §6; parses request input from the network same as `src/ingest/`, so
//! it carries the same deny-lints (§2 hardening #1).
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::indexing_slicing)]

pub mod traces;

use std::sync::Arc;

use axum::routing::get;
use axum::Router;
use parking_lot::RwLock;

use crate::store::Store;

pub fn router(store: Arc<RwLock<Store>>) -> Router {
    Router::new()
        .route("/api/traces", get(traces::list_traces))
        .route("/api/traces/{id}", get(traces::get_trace))
        .with_state(store)
}
