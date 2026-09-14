# spanfall

A local-first OTLP trace viewer, linter, and exporter, written in Rust.

Point an OpenTelemetry SDK at it, `spanfall` ingests the traces into memory and gives you a CLI
to inspect what arrived — no UI, no database, no cloud backend required.

## Status

Early and under active development. What works today:

- OTLP ingestion over gRPC (`:4317`) and HTTP/protobuf and HTTP/JSON (`:4318`), with automatic
  protocol detection on `:4317` so a misdirected HTTP client gets a clear error instead of a
  hang.
- An in-memory trace store with a configurable retention cap and oldest-first eviction.
- A JSON Read API (`:5317`) for listing traces, fetching a trace's spans, and fetching a span's
  attributes on demand.
- A CLI (`list`, `--dump`, `--demo`, `--max-memory`, `--ingest-timeout`) for inspecting resident
  state without a browser.
- Automatic port-conflict detection: if `4317`/`4318` is already bound, the error names the
  process holding it.

Not built yet: the web UI, the instrumentation linter, and single-file HTML export. See
[`docs/prd.md`](docs/prd.md) for the full product plan and [`docs/specs/`](docs/specs) for the
per-milestone engineering specs.

## Quickstart

Requires a recent stable Rust toolchain.

```sh
cargo build --release
./target/release/spanfall
```

```
Listening on http://localhost:5317
OTLP gRPC: 4317   OTLP HTTP: 4318
```

Point any OpenTelemetry SDK's OTLP exporter at `localhost:4317` (gRPC) or `localhost:4318`
(HTTP), then inspect what arrived from another terminal:

```sh
spanfall list                # resident traces: id, span count, duration, services
spanfall --dump               # everything resident as JSON: spans, flags, rejects, counters
```

To try it with no real traffic:

```sh
spanfall --demo               # starts pre-seeded with a curated fake trace set
```

## CLI

| Command | What it does |
|---|---|
| `spanfall` | Start the server: OTLP ingest on `4317`/`4318`, Read API on `5317`. |
| `spanfall list` | List resident traces on an already-running instance. |
| `spanfall --dump` | Dump all resident state (spans, flags, rejects, counters) as JSON and exit. |
| `spanfall --demo` | Start pre-seeded with a curated fake trace set instead of waiting for real ingest. |
| `spanfall --max-memory <bytes>` | Retention cap (default 1 GiB). |
| `spanfall --ingest-timeout <seconds>` | How long a blocked ingest send waits before returning a retryable reject (default 10). |

`list` and `--dump` are thin clients against an already-running instance's Read API — resident
traces live only in that process's memory, so they need a server to talk to.

## Architecture

- **Ingest** (`src/ingest/`) decodes OTLP over gRPC and HTTP (protobuf and JSON), converts it to
  an internal span representation, and hands batches to a single writer task.
- **Store** (`src/store/`) is an in-memory, trace-partitioned store (`HashMap<TraceId, Trace>`)
  with string interning for span names, attribute keys, and service names, and oldest-activity
  eviction under a memory budget.
- **Read API** (`src/api/`) serves resident state as JSON: trace list, per-trace columnar span
  payloads, per-span attributes on demand, and receiver diagnostics (arrival counts, rejects,
  counters).
- **CLI** (`src/cli.rs`, `src/main.rs`) wires the above together and provides `list`/`--dump` as
  local HTTP clients against the Read API.

Design rationale and rejected alternatives live in [`docs/prd.md`](docs/prd.md); per-milestone
specs with the concrete acceptance criteria live in [`docs/specs/`](docs/specs).

## Development

```sh
cargo build --all-targets
cargo test
cargo clippy --all-targets -- -D warnings
```

`scripts/producer.py` sends realistic traces via the real OpenTelemetry Python SDK, useful for
manual testing against a running instance — see the script for setup.

## License

Apache-2.0. See [LICENSE](LICENSE).
