//! The CLI surface (spec 07). Name and version come from Cargo metadata via clap's defaults
//! (`CARGO_PKG_NAME`/`CARGO_PKG_VERSION`), never hardcoded -- STOP #1, acceptance #6.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about = "A local-first OTLP trace viewer, linter, and exporter.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Retention cap, in bytes (default 1 GiB, STOP #5).
    #[arg(long, default_value_t = 1 << 30)]
    pub max_memory: usize,

    /// Seconds a blocked ingest send waits before returning a retryable reject.
    #[arg(long, default_value_t = 10)]
    pub ingest_timeout: u64,

    /// Dump all resident state (spans, flags, rejects, counters) as JSON to stdout and exit.
    /// Requires an already-running instance -- connects to its Read API on :5317.
    #[arg(long)]
    pub dump: bool,

    /// Start pre-seeded with a curated fake trace set instead of waiting for real ingest.
    #[arg(long)]
    pub demo: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// List resident traces on an already-running instance.
    List,
}
