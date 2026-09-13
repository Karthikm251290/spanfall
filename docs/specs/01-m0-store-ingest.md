# Spec 01 — M0: ingest, store, receiver data

**Tasks:** T4 (dedupe). The rest of this spec has **no task number** — its source is architecture
prose in `docs/prd.md` §Architecture (v1), decided 2026-09-13 and stress-tested by an Opus pass, two
adversarial rounds and an outside-voice pass. It was never re-opened.

**Owns:** `src/ingest/`, `src/store/trace.rs`, `src/store/writer.rs`, `src/store/mod.rs`,
`src/receiver.rs`, `src/api/`
**Preconditions:** T0 resolved enough to have a `Cargo.toml`.
**Blocks:** everything. `docs/prd.md` §Feature Dependencies names the span schema as *the* blocking dependency for
Stage 1, because ingest, filtering, eviction and the API payload all depend on its exact shape.

> **Read this spec with more care than the others.** It is the only one with no task list behind
> it, which means it is the one where invention creeps in unnoticed. Where it states a number, a
> type, or a string, that value was decided. Where it says *undecided*, stop.

---

## The organising insight — do not optimise the wrong thing

A 40k-span trace is roughly 8–16 MB, and a linear scan over 40k rows of packed integers in Rust is
microseconds. **There is no query-engine problem here.** The `<100ms` filter budget is spent on
payload size, parsing, and DOM layout. So: optimise the payload and the render path, and keep the
store boring.

This is why DuckDB, DataFusion, Arrow-in-memory and otel-arrow were all rejected by name
(`docs/prd.md` §Decided Against). Do not reintroduce any of them. The store is *deliberately* simpler than
the alternative it replaced.

---

## Dependency surface — the whole v1 list

`tonic`, `prost`, `opentelemetry-proto`, `axum`, `tower-http`, `tokio`, `parking_lot`,
`rustc-hash`, `clap`, `rust-embed`, `serde_json`.

Pure Rust, so cross-compiling to macOS arm64 and Linux x86_64 is uninteresting — which was the
original reason for rejecting DuckDB. **Adding a native-linking dependency breaks the two-platform
single-binary release**, which is a week 7–8 deliverable. Adding anything to this list is a
decision to surface, not absorb.

`panic = "unwind"` **must** be set in the release profile. Do not set `panic = "abort"` for binary
size: the `catch_unwind` guarantee in the ingest path silently stops working, with no compile error
and no test failure unless a test specifically covers it (`docs/prd.md` §Before Finalizing).

**This guarantee needs a release-profile check, not just a test.** `cargo test` builds the `test`
profile, which is `panic = "unwind"` regardless of what `[profile.release]` says — so acceptance
test #7 below passes even if someone later sets `panic = "abort"` on release. CI must add a step
that builds `--release` and runs a small panic-injection smoke check against *that* binary (e.g.
`--dump` against a payload that trips the ingest handler's `catch_unwind`, asserting the process
survives and returns a typed reject rather than crashing). Without this, the release profile can
silently regress and nothing turns red.

---

## 1. Span schema — `src/store/trace.rs`

The store is **trace-partitioned**: `HashMap<TraceId, Trace>`. Opening a trace is a hash lookup
plus a slice, not a scan. There is deliberately **no cross-trace index** — cross-trace search is
out of scope for v1 and the tripwire is documented in `TODOS.md`.

Each `Trace` holds its spans as **struct-of-arrays**: parallel `Vec`s, one per field, indexed by a
local span index `0..n`. Not `Vec<Span>`.

### Attributes are CSR-ragged

Not nested structs, and not one column per key. The layout is compressed-sparse-row:

- one dense array of attribute keys
- one dense array of attribute values, parallel to it
- one per-span offsets array, so span `i`'s attributes are the half-open range
  `offsets[i]..offsets[i+1]`

This is what makes "fetch the attributes for span `i`" a slice rather than a search, and it is what
makes the whole-trace payload able to *omit* attributes entirely (see §6).

### String storage — the OOM guard

This matters because **bad instrumentation is the input this tool exists to diagnose, so it must
not be able to OOM the tool** (`docs/prd.md` §Risks). A global interner over span names is exactly the
wrong choice: `GET /user/8f3a-…`-style names are unbounded by design.

| Kind of string | Where it lives |
|---|---|
| Span names, attribute **values**, status messages | A **per-trace arena**, freed when the trace is evicted |
| Attribute **keys**, service names | Interned **globally**, with a **hard cap** that raises a Receiver warning instead of growing |

The cap raising a warning rather than growing is the point: the failure is visible, bounded, and
reported as data.

### Parent resolution at insert, with patching

`parent_idx` is resolved to a **local index** at insert time.

Out-of-order arrival is the **normal path** in OTLP, not an error. When a span's parent has not been
seen yet, queue the child and **patch it in place** when the parent arrives. Consequences, both
deliberate:

- "broken parent link" becomes a **free, self-healing computed property** rather than a rejection
- a span whose parent is never seen **renders as a root, flagged** — never rejected

### T4 — dedupe on `(trace_id, span_id)`

**Effort:** ~3h human / ~30min CC.

Dedupe at insert, riding the `span_id` probe that parent resolution already performs — so this
costs no additional hash lookup.

Why it exists: a 429 makes the exporter re-send the batch (`docs/prd.md` §Architecture (v1) → Backpressure), and without dedupe
the waterfall shows every retried span **twice**. This was one of two critical gaps the engineering
review found, and it is fixed in the store so every consumer benefits rather than in any one
handler.

Dropped duplicates increment a **`duplicate_span` reject counter**, so retry storms surface as
data rather than as a mystery. Note `duplicate_span` is a **reject reason, not a span flag**.

> **STOP #4 — undecided.** Last-write-wins vs drop-on-duplicate. OTLP permits a legitimate
> re-export of the same span id with *updated* fields, so the two choices differ in observable
> output. This was recorded as an open sub-question of T4 and never resolved. Ask.

### Generation counter

Eviction plus re-arrival means a trace can be evicted and then recreated, resident again with
fresh indices `0..n` — the `RESURRECTED` case. A stored `(trace_id, span_idx)` pair therefore
navigates to a *different span* silently.

Keep a **per-trace generation counter**, compared when a stored span reference is dereferenced;
on mismatch the reference renders as plain text rather than a link.

---

## 2. `ingest::convert` — the parse contract

**One shared `convert`** is called by both transports, so rejection semantics are identical
regardless of which port a client hits. This is not an optimisation; it is the reason a
wrong-port test and a right-port test can share expectations.

```
convert(payload) -> Result<SpanBatch, RejectReason>
```

The distinction that carries the whole contract:

- **Envelope failures are errors.** A truncated protobuf or malformed JSON returns `Err`.
- **Per-span problems are flags, never errors.** One malformed span never discards the batch
  around it. The accepted spans are kept and each carries its own flags.

### Reject reasons (envelope-level, counted)

`EmptyPayload` — the exporter connected but the sampler dropped everything. This must be
**reported**, not shown as a silent blank screen; it is a named first-run failure mode.
`DecodeError` — truncated protobuf, malformed JSON. **Typed `Result`, never a panic.**
`duplicate_span` — from T4.
`late_span_after_eviction` — a span arrived for an already-evicted trace. **Counted, never
silently dropped.**

### Per-span flags (the span is accepted)

`ORPHAN` / broken parent link — parent never seen. Renders as a root, flagged.
`unknown_service` / missing `service.name` — accepted and flagged.
`PartialBatch` — some spans in the batch converted and some did not; accepted spans kept.
`RESURRECTED` — this trace was evicted and has come back.

### Hardening, all three mandatory

1. `unwrap`, `expect`, `panic` and direct indexing are **`deny`-linted in the ingest module.** Not
   a code-review convention — a lint that fails the build.
2. **Untrusted-timestamp arithmetic is checked.** Span timestamps are attacker-influenced input;
   an unchecked subtraction overflows.
3. A **`catch_unwind` at the handler boundary** maps any panic to a visible reject. A crash here is
   worse than useless in a debugging tool: the developer concludes their instrumentation is fine.

---

## 3. Transports — `src/ingest/grpc.rs`, `src/ingest/http.rs`

- **4317** — OTLP/gRPC via `tonic`.
- **4318** — OTLP/HTTP via `axum`, both **protobuf and JSON** bodies.

Both decode **in their own request handler**, so decoding runs parallel across cores, then hand the
decoded batch to the single writer task over a bounded `mpsc`. Interning and appending are serial
and cheap, so they belong behind the channel; decoding is neither and does not.

### Protocol sniffing — a headline feature, not an error path

Each listener `peek()`s the first bytes **non-destructively**. OTLP/HTTP sent to :4317, or gRPC
sent to :4318, gets a **400 whose body *is* the fix instruction**.

The reason this is a headline feature: the developer sees it **in their own exporter's log, without
opening the UI**. It is the single most documented OTel confusion in the research Evidence. A 400
with a generic message would fail the actual requirement.

---

## 4. Concurrency and backpressure

**One `parking_lot::RwLock<Store>`.** Not `arc-swap` — swapping an immutable store would require
cloning every span on every batch, which is the wrong trade for an append-heavy store.

Lock discipline, both halves load-bearing:

- The **writer** holds the write lock for microseconds per batch.
- **Readers copy out the rows they need and release the lock before serialising.** Hold the read
  lock for the memcpy, never for serialisation.

### Backpressure — bounded, never dropping

`send().await` blocks. After `--ingest-timeout` (default **10s**, overridable) it returns a
**retryable** error:

| Transport | Response |
|---|---|
| gRPC | `RESOURCE_EXHAUSTED` |
| HTTP | `429` + `Retry-After` |

The SDK's own queue absorbs the retry. **Never a silent drop; never an unbounded hang.** Both of
those are worse than a 429, because both are invisible.

This is the mechanism that makes T4 necessary: the retry is *correct* behaviour, and dedupe is what
keeps it from double-rendering.

---

## 5. Retention and eviction

Capped **by memory (`--max-memory`), not by span count**, because bytes/span varies ~5x with
attribute density.

> **STOP #5 — undecided.** The default: 512 MiB, 1 GiB, or 2 GiB. The assumption is 1 GiB, to be
> pinned after M1 measures actual bytes/span (`docs/prd.md` §Open Questions → `--max-memory` row). Ask, or implement the flag with
> no default and require it until M1 lands.

Eviction drops the trace with the **oldest last-activity** — the quietest trace. **Never one still
receiving spans.** A long-running trace that is still getting spans must survive eviction; that is
an explicit test case.

**"Complete" is deliberately undefined.** OTLP provides no completion signal, and any timeout-based
definition would be a lie. Do not add one.

Eviction counters and the `RESURRECTED` flag are surfaced in the UI. A span arriving for an evicted
trace is **counted** (`late_span_after_eviction`), never silently dropped.

---

## 6. Read API — `src/api/`

Contract defined here because the server owns it; the waterfall that consumes it is M2 (spec 05)
and the client seam is spec 04. **Defining it in two places is how the two drift.**

| Route | Returns | Hard constraint |
|---|---|---|
| `GET /api/traces` | Trace summaries | Carries span count, duration and service list — spec 05's skeleton renders from this before the trace payload arrives |
| `GET /api/traces/:id` | **Columnar JSON + string table**, gzipped via `tower-http` | **MUST NOT contain span attributes.** This is the 40k-span path and the payload-discipline claim. |
| `GET /api/traces/:id/spans/:idx/attributes` | One span's attributes | Fetched on click. A slice out of the CSR arrays. |
| `GET /api/traces/:id/filter?q=` | **Span indices only** | Never span objects. The browser masks rows it already holds. |
| `GET /api/receiver` | Counters + recent rejects | Includes `duplicate_span`. Positive signal first (see §7). |
| `GET /api/events` | SSE, **invalidation only**: `{"type":"changed","seq":N}` | **No span data on this stream.** One rendering path, not separate initial-load and streaming-update code. |
| `GET /api/lint` | Lint findings | Spec 03 owns the body shape. |

### Trace payload shape

**Columnar JSON with a string table**: parallel numeric arrays, names as indices into the table, no
repeated object keys. Roughly 5x smaller than per-span objects and parsed on the JS fast path.

Explicitly **not Arrow IPC** — rejected because it is a large JS dependency for a format the browser
must unpack into typed arrays anyway.

Documented escape hatch if M1 says this is still too slow: little-endian packed binary read via
`DataView`, still zero new dependencies. Do not build it pre-emptively; it is funded by an M1
result (spec 02).

### Filter

A hand-rolled predicate scan (~150 LOC) plus a whitespace-separated grammar. The literal example
from `docs/prd.md` §Architecture (v1) → Query layer:

```
service=checkout status=error http.status_code>=500 duration>100ms "cart"
```

Not SQL — nobody wants to write `WHERE` clauses per keystroke. **One filter implementation, in
Rust**, serving in-trace filtering now and cross-trace search later. A second implementation in JS
is rejected by name in `docs/prd.md` "Decided Against", and that rejection is load-bearing for
spec 06 (no filter box in exports).

---

## 7. Receiver data — `src/receiver.rs`

A struct of counters plus a ring buffer, produced as a byproduct of ingest. **The data lands in M0,
not M3.** M3 is only the *view* over it, budgeted at 1–2 days.

Two risks closed by moving the data early: M3 landing days before launch with no bug-fix buffer,
and the M1 gate having no fallback differentiator if the performance thesis fails.

**Most of its value is *positive* signal.** Lead with "412 spans from `checkout` over
HTTP/protobuf, last seen 3s ago" — not with a reject list. A panel that only shows failures is
useless in the common case where everything works and the developer is looking in the wrong place.

Contents: per-endpoint and per-protocol arrival counts, resource attributes seen, payload counts,
malformed/rejected records with reasons, spans with missing or broken parent links, eviction
counters, `late_span_after_eviction`, `duplicate_span`.

The rejects **ring buffer** wraps oldest-first at capacity. That wraparound is a test case.

---

## 8. `--dump`

M0's exit criterion is that **all of the above is provable via `--dump`**, including span flags and
rejects. It exists so M0 can be verified with no UI, and so T1-style analysis has a path that does
not require a browser.

---

## Acceptance

Cite from
`~/.gstack/projects/spanfall/karthik-no-branch-eng-review-test-plan-20260913-122923.md`:
"Key Interactions" rows 1–3, and "Edge Cases" — the parent-never-seen row, missing `service.name`,
partial batch, truncated protobuf, 15k burst, memory-cap boundary, evicted-trace span, long-running
trace not evicted, ring-buffer wraparound, retried-429 dedupe, port-already-bound.

Spec-specific additions:

| # | Check |
|---|---|
| 1 | An SDK pointed at 4317 and at 4318 (protobuf **and** JSON) produces spans in <60s with **zero config files**. |
| 2 | OTLP/HTTP sent to :4317 returns 400 whose **body carries the fix**, readable in the exporter's own log with the UI never opened. |
| 3 | `GET /api/traces/:id` response contains **no attributes**. Assert on the payload, not on the UI. |
| 4 | `GET /api/traces/:id/filter?q=` returns indices, never span objects. |
| 5 | `GET /api/events` carries no span data. |
| 6 | Retried batch after a 429: span count unchanged, `duplicate_span` incremented, nothing rendered twice. |
| 7 | Truncated protobuf and malformed JSON both return a typed reject. Verified **with `panic = "unwind"` in the release profile** — under `abort` this test silently stops proving anything. |
| 8 | A span whose parent never arrives renders as a root and is flagged; when the parent arrives late the link patches in place. |
| 9 | Global interner cap reached → Receiver warning, no unbounded growth. |
| 10 | A long-running trace still receiving spans is not evicted while quieter traces are. |
| 11 | Zero spans received: the Receiver reports `EmptyPayload` where applicable, and never a silent blank. |

## Out of scope

Cross-trace search or any cross-trace index. Any embedded query engine. Persistence (Stage 2,
length-delimited OTLP protobuf, **not** Parquet). Logs and metrics. Auth on the HTTP API —
`127.0.0.1` bind only. The packed-binary payload escape hatch. The Receiver *panel* (M3, a view
over this data). Sampling logic of any kind: it shows what it receives.

## STOP items live here

**#4** dedupe semantics (T4) and **#5** `--max-memory` default. **#2** — the UI port is named
nowhere in any document; this spec's server binds it, so it surfaces here first.
