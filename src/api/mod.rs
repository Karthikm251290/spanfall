//! The Read API, served on the UI port (`:5317`, STOP #2), separate from the OTLP ingest ports.
//! Route contract is spec 01 §6; parses request input from the network same as `src/ingest/`, so
//! it carries the same deny-lints (§2 hardening #1).
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::indexing_slicing)]

pub mod attributes;
pub mod filter;
pub mod receiver;
pub mod traces;

use std::sync::Arc;

use axum::extract::FromRef;
use axum::routing::get;
use axum::Router;
use parking_lot::RwLock;

use crate::ingest::receiver_state::ReceiverState;
use crate::store::Store;

/// Combined axum state for the whole Read API. Each handler extracts only the piece it needs
/// via `FromRef` (below), so existing single-`State<Arc<RwLock<Store>>>` handlers didn't need to
/// change when `/api/receiver` needed a second, ingest-layer piece of state.
#[derive(Clone)]
pub struct ApiState {
    pub store: Arc<RwLock<Store>>,
    pub receiver: Arc<RwLock<ReceiverState>>,
}

impl FromRef<ApiState> for Arc<RwLock<Store>> {
    fn from_ref(state: &ApiState) -> Self {
        state.store.clone()
    }
}

impl FromRef<ApiState> for Arc<RwLock<ReceiverState>> {
    fn from_ref(state: &ApiState) -> Self {
        state.receiver.clone()
    }
}

#[cfg(test)]
pub(crate) fn test_state(store: Store) -> ApiState {
    ApiState { store: Arc::new(RwLock::new(store)), receiver: Arc::new(RwLock::new(ReceiverState::default())) }
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/api/traces", get(traces::list_traces))
        .route("/api/traces/{id}", get(traces::get_trace))
        .route("/api/traces/{id}/spans/{idx}/attributes", get(attributes::get_attributes))
        .route("/api/traces/{id}/filter", get(filter::get_filter))
        .route("/api/receiver", get(receiver::get_receiver))
        .with_state(state)
}
