//! Envelope failures are errors; per-span problems are flags, never errors (§2,
//! `01-m0-store-ingest.md`). `unwrap`/`expect`/`panic`/direct indexing are deny-linted here — not
//! a review convention, a build-failing lint. `cargo build` alone does not enforce this; it needs
//! `cargo clippy -- -D warnings` in CI, which is not yet wired up.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::indexing_slicing)]

pub mod convert;
pub mod grpc;
pub mod handler;
pub mod http;
pub mod receiver_state;
pub mod sniff;
