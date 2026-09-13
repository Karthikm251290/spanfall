---
id: 01M2CHBP4RM7XYFD5N2SAAEA88
type: prd
title: "Local OpenTelemetry Trace Viewer (Rust) — v1"
created_at: 2026-09-13T04:45:40Z
skill: prd-generator
work_area: specs
derived_from: "Research report: Local OpenTelemetry Viewer — Pain Points, Competitive Gaps, and v1 Design Validation (2026-09-13)"
tags: [rust, opentelemetry, observability, developer-tools, oss]
---

# PRD: Local OpenTelemetry Trace Viewer — working name `tracescope`

**Status:** Draft — Architecture Decided (v1 storage/query design settled 2026-09-13 via `/plan-eng-review`; see Architecture and "Decided Against" below)
**Owner:** Karthik
**Last Updated:** 2026-09-13
**Target Release:** v1 (Stage 1) — 9–10 weeks from first commit (serious push, not weekend pace). Was 6–8; re-costed 2026-09-13 when the CEO review's scope expansions (linter, HTML export, demo) added ~2.3 weeks that had never been added to the milestone table.
**Availability:** Open source, public GitHub repo. Apache-2.0 or MIT (decide before first commit).
**Rationale:** Priority order is explicit and stated: **(1) credibility asset for the job search, (2) a tool people actually use, (3) deep Rust learning.** When these conflict, the earlier one wins. Practically: a publishable benchmark result beats another feature, and the boring idiomatic Rust path beats the instructive one. Permissive licensing removes adoption friction, which is the whole point of the wedge.

---

## Context

*No `context/product.md`, `personas.md`, `company.md`, or `competitors.md` exist in this workspace. Everything below is grounded in the research report of 2026-09-13 and in decisions stated in conversation. Where I am inferring rather than citing, it is marked ⚠️.*

- **Roadmap:** Not on any roadmap — this is a net-new personal OSS project, chosen from a shortlist of three Rust ideas.
- **Strategic fit:** Flagship public repo demonstrating real systems engineering in Rust, built primarily as a credibility asset for the job search. Stated priority: credibility → real usage → Rust learning. Stars are a proxy for the first two, not the goal itself.
- **Competitive:** The local-viewer space is occupied but not won. otel-desktop-viewer (~1.2k stars, Go/DuckDB/Svelte) and otel-tui (~1k stars, Go/TUI) lead; Microsoft's Aspire Dashboard is polished but in-memory only. **No established Rust entrant exists.** Incumbents are healthy, so "another local viewer" is not a thesis — the thesis must be performance on large traces plus persistence and diffing.
- **Adjacent but not competing:** SigNoz (~26k stars) and Grafana Tempo are production backends. Competing with them is a losing fight and explicitly out of scope.

---

## Problem

Developers instrumenting an application with OpenTelemetry cannot easily see their own telemetry. To view a trace locally they must either stand up a multi-container observability stack (Collector + Jaeger or Grafana + Tempo + Prometheus) or ship data to a vendor backend. Both are disproportionate to the actual need: *"is my instrumentation working, and what did this request actually do?"*

Two failure modes compound it:

1. **Setup friction.** When traces don't appear, the developer has no local signal explaining why — wrong port (4317 gRPC vs 4318 HTTP), misconfigured exporter, sampling dropping spans, broken context propagation, or `unknown_service` resource attributes. The debugging loop is blind.
2. **Large-trace collapse.** Once traces get big — thousands to tens of thousands of spans, which is normal for batch jobs, fan-out services, and LLM agent loops — the incumbent UIs become unusable rather than merely slow.

**Who has it:** Primarily the individual developer debugging instrumentation on a laptop during development, and by extension small teams on shared dev/staging.

---

## Evidence

### ✅ Validated — from primary sources

**The "just let me see my traces" need is documented by OpenTelemetry itself.**
- The OTel Developer Experience SIG survey (published 2025-04-02; 218 responses, 83% running OTel in production, 77.4% non-vendor) concluded that a common theme was the desire for an easier-to-set-up local environment where you can see all telemetry from a running application locally, plus better SDK signals when data is being dropped or an exporter is misconfigured.
- Microsoft MVP Anthony Simmon, on why the Aspire dashboard exists: the prior docker-compose recommendation "was just too much… most developers simply want to visualize their application telemetry, not learn to become YAML engineers."

**Exporter/config troubleshooting is the most painful part of OTel.**
- DevEx survey: SDK configuration is complex and troubleshooting the exporter is the most painful part; it is difficult to know why exporting isn't working or whether the SDK is dropping data, and better local debugging tooling would help.
- Collector-contrib issue #36116: Collector configuration "presents a significant barrier to entry" with a "steep learning curve." Community guidance concedes most Collector problems are YAML problems, not code problems.

**Large-trace rendering is a real, unresolved failure in the incumbents.**
- Jaeger UI #645: traces above ~2000 spans make the UI extremely slow — clicking a single span to see tags takes about five seconds.
- Jaeger UI #562: traces of ~40K spans do not load at all, because opening a trace loads and expands every span.
- Jaeger UI #186: after a large query, typing in the tag filter costs several seconds per character. Jaeger #178 and #2693 document related payload-size and first-query-latency problems.
- Grafana community forum: a 30k-span trace does not open in the Grafana UI. Grafana #114264: the Explore trace panel regressed to 1–2 minutes to load a trace due to ~4x tag-value request amplification.
- Grafana's own Tempo troubleshooting docs advise shrinking time ranges and span limits because large traces or result sets consume excessive memory.

**Even the leading local viewers admit performance ceilings.**
- otel-tui's author states the motivation directly: with significant data volume over time, otel-desktop-viewer "can become sluggish in terms of query and screen rendering."

**Persistence and diffing are genuine gaps.**
- Aspire Dashboard stores telemetry in memory only — all data disappears when the container restarts.
- The only local tool that advertised side-by-side trace comparison (otel-front) is **archived**, with efforts folded into otel-desktop-viewer.

**Jaeger v2 does not close the UI gap yet.**
- Jaeger v2 went GA 2024-11-12 on the OTel Collector framework; v1 reached EOL 2025-12-31. Maintainers list "upgrading the UI to use OpenTelemetry data natively" as a *future* roadmap item, and community reaction to v2 was reserved specifically on UI improvements.

**Rust is capable of this.**
- `opentelemetry-proto` (with `gen-tonic`) provides generated OTLP types and gRPC scaffolding; `tonic`/`prost` serve OTLP-gRPC on 4317; `axum`/`hyper` serve OTLP-HTTP protobuf and JSON on 4318.
- Embedded columnar OLAP crates exist (`duckdb` bundled, Apache DataFusion over `arrow-rs`) and were evaluated, but the v1 workload does not need one — see "Decided Against". `parking_lot` and `rustc-hash` cover the concurrency and hashing this design actually uses. `ratatui` (~22.5k stars) is mature if a TUI mode is ever wanted.

**Export/escape-the-vendor demand exists (relevant to Stage 3).**
- Grafana #61377 requests a button to export the current trace as JSON, explicitly to make sharing traces during troubleshooting easier.
- `ddexport` exists because Datadog's UI only lets you export 5000 rows of logs or spans.
- Cost anger is well documented — the ~$65M Datadog bill was confirmed on Datadog's Q1 2023 earnings call (2023-05-04), with the CFO acknowledging a crypto customer.

### ⚠️ Assumed — flag for validation

- ⚠️ **That a Rust implementation will materially outperform otel-desktop-viewer and Jaeger on 10k–40k-span traces.** Plausible given architecture choices, but unproven. This is the load-bearing assumption of the entire project.
- ⚠️ **That developers want to pull production traces from cloud backends into a local viewer.** The research found strong *export-and-share* and *lock-in/cost* signals but **no direct, high-volume request** for this specific workflow. Treat as hypothesis, not fact.
- ⚠️ **That the segment is large enough to matter.** otel-desktop-viewer at ~1.2k stars suggests a real but modest audience for local viewers specifically.
- ⚠️ **That trace-log correlation is worth v1.1 effort.** Evidence is good (otel-tui made log→trace jump a headline feature; teams describe copy-pasting context between logs and traces) but not quantified.
- ⚠️ **That CI/test-run trace analysis is a viable later segment.** Evidence here is thin.
- ⚠️ Competitor star counts and maintenance status were observed at research time and drift. Re-verify before publishing any comparison table.

---

## Success Criteria

Indicators are weighted by the stated priority order. A public benchmark result that survives technical scrutiny is worth more than raw star count; star count is worth more than how much Rust got learned along the way. No baselines exist yet — this repo does not exist.

### Lagging Indicators (post-launch outcomes)

| Metric | Current | Target | Timeframe |
|---|---|---|---|
| GitHub stars | 0 | 250+ (credibility threshold), 1k+ (peer-of-incumbents) | 6 months post-launch |
| Issues opened by people who are not the author | 0 | ≥10 substantive issues | 3 months post-launch |
| External contributors (merged PR) | 0 | ≥3 | 6 months post-launch |
| Unprompted mention in an OTel community channel, newsletter, or awesome-list | 0 | ≥1 | 6 months post-launch |
| Benchmark claims survive public scrutiny (HN/Reddit) without being credibly debunked | n/a | No successful methodology challenge | At launch + 2 weeks |
| Repo cited in a job conversation or interview | 0 | ≥1 | [PLACEHOLDER — tied to job-search timeline] |

### Leading Indicators (pre-launch signals)

| Metric | Current | Target | What This Predicts |
|---|---|---|---|
| **Time-to-first-trace** from `curl \| sh` to a span on screen | n/a | < 60 seconds, zero config files | Adoption — this is the entire pitch |
| **10k-span trace**: time to open + first interaction | n/a | < 1s to first paint, < 100ms filter keystroke — **both measured in a real browser via `performance.now()`**, not backend-only | Whether the differentiator is real |
| **40k-span trace**: opens and remains interactive | n/a | Exporter flush → visible and interactive < 3s, no hang (the exact case Jaeger #562 fails). **This end-to-end number is the one published.** | Whether the differentiator is real |
| Writer append throughput (serial path) | n/a | Must outpace realistic exporter load — measured, not asserted | Whether "shows what it receives, no sampling" is actually true. If normal load triggers a 429, the claim is false. |
| Bytes per span (this exact column layout) | n/a | Measured in M1; estimate 250–400 B once attributes are counted. Sets the `--max-memory` default. | Laptop viability — replaces the earlier span-count placeholder, since bytes/span varies ~5x with attribute density |
| Binary size / install steps | n/a | Single binary, ≤1 step, no Docker | Friction |
| Dogfood usage on own projects (slab, NomNom-shaped services) | 0 | Used weekly for 4 consecutive weeks before launch | Whether it's actually useful vs. merely impressive |

💡 **The two trace-size rows are the go/no-go gate.** If a Rust implementation cannot beat Jaeger and otel-desktop-viewer on 10k and 40k spans, the core differentiator is gone and the project should be re-scoped toward diffing and cloud ingestion — or shelved.

---

## Proposed Solution

### How It Works

A single statically-linked binary. Run it with no arguments. It immediately listens on the two ports developers already know — 4317 (OTLP/gRPC) and 4318 (OTLP/HTTP, protobuf and JSON) — and serves a local web UI on a third port. Point any OTel SDK at `localhost` and traces appear live.

Three things distinguish it from every incumbent:

1. **It stays fast when traces get big.** Trace-partitioned columnar storage plus a virtualized waterfall that renders only what's on screen — and, the part that actually matters, it never ships a payload it doesn't need. Span attributes are fetched per span on click, not bundled into the trace payload. Every incumbent failure cited in Evidence (Jaeger #645, #562, #186; Grafana #114264) is a payload-amplification or rendering failure, not a storage failure. The architecture targets that.
2. **It tells you why nothing showed up.** A first-class "Receiver" panel showing raw arrivals: which endpoint and protocol, resource attributes, payload counts, malformed or rejected records, and spans with missing or broken parent links. Most of its value is *positive* signal — "yes, 412 spans from `checkout` over HTTP/protobuf, last seen 3s ago" — not just a reject list. And the most common misconfiguration never needs the panel at all: send OTLP/HTTP to the gRPC port and the 400 response body tells you the fix, in your own exporter's log. This targets the exact complaint OTel's own survey surfaced.
3. **It remembers, and it compares.** Optional on-disk persistence so traces survive a restart, plus trace diffing — pick two traces of the same operation and see structural and timing deltas.

**Architecture (v1):**

Decided 2026-09-13. The organizing insight: a 40k-span trace is roughly 8–16 MB, and a linear scan over 40k rows of packed integers in Rust is microseconds. There is no query-engine problem here. The `<100ms` filter budget is spent on payload size, parsing, and DOM layout — so the architecture optimizes the payload and the render path, and keeps the store boring.

- **Ingest:** `tonic` (gRPC/4317) and `axum` (HTTP/4318) over `opentelemetry-proto` types. Both transports decode in their own request handler (parallel across cores), then hand a decoded batch to a single writer task over a bounded `mpsc` channel (interning and appending are serial and cheap). Both call one shared `ingest::convert`, so rejection semantics are identical regardless of which port a client hits.
- **Protocol sniffing:** each listener `peek()`s the first bytes non-destructively. OTLP/HTTP sent to :4317 (or gRPC to :4318) gets a 400 whose body *is* the fix instruction, so the developer sees it in their own exporter log without opening the UI. This is a headline feature, not an error path — it is the single most documented OTel confusion in Evidence.
- **Store:** trace-partitioned, `HashMap<TraceId, Trace>` where each `Trace` holds its spans as struct-of-arrays. Opening a trace is a hash lookup plus a slice, not a scan. Attributes are stored CSR-ragged (parallel dense key/value arrays plus per-span offsets), not nested structs and not one column per key. Spans are deduped on `(trace_id, span_id)` at insert — the probe parent resolution already performs — because a 429 makes the exporter re-send the batch and duplicates would otherwise render twice; dropped duplicates increment a `duplicate_span` reject counter so retry storms surface as data. `parent_idx` is resolved to a local index at insert, with unresolved parents queued and patched in place when the parent arrives — so out-of-order arrival is the normal path and "broken parent link" becomes a free, self-healing computed property rather than a rejection.
- **Concurrency:** one `parking_lot::RwLock<Store>`. The writer holds the write lock for microseconds per batch; readers copy out the rows they need and release before serializing. (Not `arc-swap`: swapping an immutable store would require cloning every span on every batch.)
- **Backpressure:** bounded. `send().await` blocks, but after `--ingest-timeout` returns a retryable gRPC `RESOURCE_EXHAUSTED` / HTTP 429 + `Retry-After`, which the SDK's own queue absorbs. Never a silent drop; never an unbounded hang.
- **Retention:** capped by memory (`--max-memory`), not span count, because bytes/span varies ~5x with attribute density. Eviction drops the trace with the oldest *last-activity* — the quietest trace, never one still receiving spans. "Complete" is deliberately undefined, because OTLP provides no completion signal and any timeout-based definition would be a lie. Eviction counters and a `RESURRECTED` flag are surfaced in the UI; a span arriving for an evicted trace is counted, never silently dropped.
- **Query layer:** a hand-rolled predicate scan (~150 LOC) plus a whitespace filter grammar (`service=checkout status=error http.status_code>=500 duration>100ms "cart"`), not SQL. Nobody wants to write `WHERE` clauses per keystroke.
- **API:** REST for all data, plus one SSE stream carrying *invalidation only* (`{"type":"changed","seq":N}`) so there is a single rendering path rather than separate initial-load and streaming-update code. `GET /api/traces/{id}/filter?q=` returns matching span indices, not spans; the browser masks rows it already holds. One filter implementation in Rust serves in-trace filtering and later cross-trace search.
- **Trace payload:** columnar JSON with a string table (parallel numeric arrays, names as indices, no repeated object keys), gzipped by `tower-http`. Roughly 5x smaller than per-span objects and parsed on the JS fast path. Escape hatch if M1 says it is still too slow: little-endian packed binary read via `DataView`, still zero new dependencies.
- **Persistence (v1.1):** length-delimited OTLP protobuf frames behind a magic + format-version header. Zero new dependencies, forward/backward compatibility for free (so adding logs in v1.1 does not break v1.0 files), and "load a stored OTLP file" reuses `convert` verbatim. Written temp-then-atomic-rename, non-fatal on failure — a full disk must not kill a live debugging session. Parquet demotes to an *export* feature where `arrow-rs` earns its dependency at the point of use.
- **Parse contract:** `convert` returns `Result<SpanBatch, RejectReason>`. Envelope failures are errors; per-span problems are flags, never errors, so one malformed span never discards the batch around it. `duplicate_span` is a reject reason, not a flag. `unwrap`/`expect`/`panic`/indexing are `deny`-linted in the ingest module, untrusted-timestamp arithmetic is checked, and a `catch_unwind` at the handler boundary maps any panic to a visible reject. Requires `panic = "unwind"` in the release profile.
- **Dependency surface (v1):** `tonic`, `prost`, `opentelemetry-proto`, `axum`, `tower-http`, `tokio`, `parking_lot`, `rustc-hash`, `clap`, `rust-embed`, `serde_json`. Pure Rust, so cross-compiling to macOS arm64 and Linux x86_64 is uninteresting — which was the original reason for rejecting DuckDB.
- **UI:** embedded SPA served by `axum` from the binary (the otel-desktop-viewer pattern), chosen over TUI because deep-zoom waterfalls, attribute panels, and diff views are impractical in a terminal.
- **TUI mode:** deferred, not rejected.

**Interface surface (v1) — settled by `/plan-design-review` 2026-09-13.** Detail of record is `docs/designs/tracescope-v1.md` (§Lint tab, §Terminal output, §Design tokens, §Keyboard); this is the summary an implementer needs before opening that file:

- **CLI:** `tracescope` (run), `tracescope lint` (**newest resident trace**), `tracescope lint --all` (everything resident), `tracescope lint <file.otlp>` (a capture, via the Stage 2 load path), `tracescope export <trace-id>`, `--redact`, `--force`, `--demo`. Lint is computed **on request** — no accumulator, no epochs, no reset — so two calls with no traffic between them return identical output.
- **Lint scope is the reason the loop closes.** Defaulting to the newest trace is what makes `✓ clean` reachable after you fix your instrumentation: the old bad traces stay resident and would otherwise keep reporting. Each of the six rules declares per-trace or cross-trace scope; the default run **names the two rules it skipped** (span-name cardinality, attribute type conflict) and prints the command that runs them. A skipped rule must never render as a pass.
- **Export:** view-only static HTML, capped at ~5k spans unless `--force`, no filter box, a provenance bar stating SNAPSHOT / source / UTC time / span count (and "N of M spans (capped)" when truncated). Span data is embedded as `<script type="application/json">` with `<` written as the six-character JSON escape `\u003c` (backslash, u, 0, 0, 3, c) — *not* HTML-escaped, which would mangle `Handler<Request>` — rendered via `textContent` only, under an additive CSP `default-src 'none'`. This is the one surface that travels to other people and cannot be patched after it is sent, so it is the surface that gets responsive layout (375px) and embedded fonts.
- **Two faces, one token block:** IBM Plex Sans for UI, IBM Plex Mono for every id, count and duration, both embedded (no network at `file://`). 15 CSS custom properties, one radius, one spacing scale; light and dark defined together under `prefers-color-scheme` with **no toggle**, because the palette shape is a build-order dependency rather than a polish item. Every severity colour measures ≥4.5:1 against `--bg` in both palettes, and severity is carried by group headings so it survives greyscale.
- **Accessibility is not deferred:** the virtualized waterfall announces as `role="tree"` with `aria-level`, `aria-expanded` and `aria-rowcount`; arrow keys walk rows with the focused row kept mounted through recycling; `/` focuses the filter, Escape clears then closes. Touch targets ≥44px.
- **The running process prints one nudge**, on the first batch of spans, then stays silent. Nothing is printed on a timer — that cadence described the deleted accumulator.
- **UI work not in the 9–10 week figure:** D1–D14 in the design doc add roughly 4–5 days. T14 re-costs the milestone table and must absorb them.

### Decided Against

Preserved with reasons so these are not re-litigated mid-build:

- **DuckDB (bundled) for storage or persistence.** `duckdb-rs`'s bundled feature compiles DuckDB from C++ source; cross-compilation is documented as best-effort and not CI-covered, which put the week 7–8 two-platform single-binary release at risk.
- **Apache DataFusion for the v1 query path.** It buys a SQL parser, optimizer, and parallel execution over a 40k-row single-user table where the query is always one of four shapes known at compile time. It costs build minutes, binary size, a planning step inside the latency budget, and it pressures the schema to be query-engine-shaped (wide, immutable batches) exactly where ingest needs it trace-shaped and mutable. Revisit only if multi-GB persisted ad-hoc SQL becomes a goal, which is an explicit Non-Goal.
- **Arrow `RecordBatch` as the in-memory format.** Immutable-once-built, which fights incremental append, in-place parent patching, and cheap whole-trace eviction. Would require a mutable staging buffer that is the struct-of-arrays anyway, plus a conversion. Arrow returns in v1.1 as the Parquet export path.
- **otel-arrow / OTAP as the internal schema.** Optimized for collector-to-collector wire compression (dictionary encoding, delta timestamps), not for trace-tree traversal and interactive filtering.
- **Arrow IPC to the browser.** A large JS dependency for a format the browser must unpack into typed arrays regardless; columnar JSON plus a string table gets most of the win at zero dependency cost.
- **Client-side filtering over a fully-hydrated trace.** Considered and reversed: it inflates the open-40k payload on the exact metric the M1 gate measures, and it forces a second filter implementation in JS whose semantics must track the Rust one used for cross-trace search.

### User Stories

**Story 1 — the instrumentation loop**
- **As a** developer adding OTel to a service for the first time
- **I want to** run one binary and immediately see whether my spans are arriving, and what's wrong if they aren't
- **So that** I stop guessing between port, exporter, sampler, and propagation as the cause
- **The loop must close without a restart.** Run `lint`, see findings, fix the instrumentation, re-run the request, run `lint` again, read **clean** — while the old bad traces are still resident. This is the single acceptance test the feature exists for, and it is why `lint` defaults to the newest trace rather than everything in the store.

**Story 2 — the big trace**
- **As a** developer debugging a fan-out request or an agent loop that produces 15,000 spans
- **I want to** open the trace, collapse subtrees, and filter by attribute without the UI freezing
- **So that** I can actually find the slow span instead of abandoning the tool

**Story 3 — the regression**
- **As a** developer who just changed a query path
- **I want to** compare today's trace of `POST /checkout` against yesterday's stored one and see which spans got slower or appeared
- **So that** I can confirm or disprove a regression in seconds rather than eyeballing two browser tabs

---

## Non-Goals

Explicitly **not** doing in v1:

- **Not a production observability backend.** No clustering, no retention policies, no multi-tenancy, no HA. SigNoz, Tempo, and Jaeger own that space.
- **Not metrics.** Deferred indefinitely pending user demand. Time-series storage, aggregation, and dashboards are a different product.
- **Not logs in v1.** Planned for v1.1 with trace correlation, not before.
- **Not an OTel Collector replacement.** No processors, no pipelines, no OTTL, no routing.
- **Not alerting, SLOs, or anomaly detection.**
- **Not auth, RBAC, or TLS-terminated multi-user deployment.** It binds to localhost and is assumed single-user.
- **Not Kubernetes deployment, Helm charts, or operators.**
- **Not cloud ingestion in v1.** Stage 3, and only after demand validation.
- **Not sampling or head/tail sampling logic.** It shows what it receives.

---

## Dependencies

### Feature Dependencies
- **OTLP protobuf definitions** (`opentelemetry-proto` crate) — stable, versioned, low risk
- ~~**Columnar storage decision**~~ — **DECIDED 2026-09-13.** Trace-partitioned native struct-of-arrays, hand-rolled predicate scan, no embedded query engine. See Architecture and "Decided Against". No longer blocking.
- **Span schema (`store/trace.rs`)** — the column layout, CSR attribute encoding, and insert-time parent resolution must be fixed before anything else is written, because ingest, filtering, eviction, and the API payload all depend on it. This is now the real blocking dependency for Stage 1.
- **Embedded SPA build pipeline** — the UI must compile into the binary (`rust-embed` or similar); affects release engineering from day one

### Team Dependencies
- **None.** Solo project. This is a risk in itself — see Risks (no design review, no second pair of eyes on UX).
- ⚠️ Optional: recruit 3–5 beta users from the OTel community before Show HN, to avoid launching untested UX.

### External Dependencies
- **`tonic`/`prost`** — mature, but gRPC version churn is a maintenance tax. These are now the heaviest dependencies in the tree; v1 takes no native-linking dependency at all, so cross-compilation is pure Rust.
- **Grafana Tempo HTTP API** (Stage 3 only) — `GET /api/traces/{traceID}`, TraceQL `GET /api/search?q=`; open and unmetered
- **Cloud provider APIs** (Stage 3 only, uneven feasibility):
  - Google Cloud Trace — 300 read units per 60s, where `ListTraces` costs 25 units and `GetTrace` costs 1; a list-then-fetch loop burns quota fast
  - Datadog — `/api/v2/spans/events`, **300 requests/hour**, requires `apm_read` plus API and APP keys, returns only retained/indexed spans
  - AWS X-Ray — `BatchGetTraces` accepts a **maximum of 5 trace IDs per call**, throttles with 429, and cannot find traces when Transaction Search is enabled; retrieval billed at $0.50 per million traces retrieved/scanned with 1M free monthly
  - Honeycomb — Query Data API returns aggregates only and is Enterprise-gated; **no documented raw-span bulk export**. Skip until proven otherwise.

**Critical Path:** The **M1 benchmark gate** — specifically its browser-measured rows. The storage/query decision that previously sat here is settled (see Architecture). What remains unproven is whether the end-to-end experience (exporter flush → 40k spans visible and interactive in a real browser) beats the incumbents. Since roughly 90% of that budget is browser-side, a gate measured only at the backend would not gate anything, which is why M1 now includes a throwaway browser spike.

---

## Risks

*Risk types: V=Value, U=Usability, F=Feasibility, B=Business Viability (here: portfolio/credibility viability). Impact: H/M/L*

| Risk | Type | Impact | Mitigation |
|---|---|---|---|
| Rust implementation fails to beat incumbents on large traces — the entire differentiator evaporates | F | **H** | Build the benchmark harness and a 40k-span synthetic generator **in week 1–2, before the UI**, plus a throwaway browser spike, because ~90% of the latency budget is browser-side and a backend-only gate gates nothing. Benchmark against the OpenTelemetry demo app as well as synthetic traces. Treat the 10k/40k targets as a hard go/no-go gate with a pre-defined pivot (see Milestone Plan). |
| Incumbents close the gap first — Jaeger v2 ships native OTel UI, or otel-desktop-viewer adds fast large-trace rendering and diffing | B | **H** | Monitor jaeger-ui and otel-desktop-viewer releases monthly. If they ship first, pivot the thesis to diffing + cloud ingestion. Ship Stage 1 fast; the window is the asset. |
| Audience too small — local viewers cap around ~1k stars, so "massive" never happens | B | M | Accept credibility-asset framing over viral framing. Star targets set accordingly (250 = success, 1k = peer-of-incumbents). Do not redesign the product chasing stars. |
| Solo build means no design review; UX ships worse than Aspire's | U | M | Steal proven interaction patterns rather than inventing (Aspire's layout, Jaeger's waterfall conventions). Recruit 3–5 beta users pre-launch. |
| The render path, not the store, is the real ceiling — DOM virtualization can't hold 40k rows interactively | F | **H** | This is now the primary technical risk (the store is provably not the bottleneck at this scale). Mitigation: M1's browser spike measures it before a framework is chosen. Pivot if it fails: single `<canvas>`, draw rects, hit-test on click — a contained renderer change, not a project pivot. |
| High-cardinality span names (`GET /user/8f3a-…`) leak memory through a global string interner | F | M | Span names, attribute *values*, and status messages live in a per-trace arena freed on eviction; only attribute *keys* and service names are interned globally, with a hard cap that raises a Receiver warning instead of growing. Bad instrumentation is the input this tool exists to diagnose, so it must not be able to OOM it. |
| Scope creep into logs/metrics/cloud before the core is proven | F | M | Non-Goals section is binding. Nothing from Stage 2 or 3 starts until the Stage 1 benchmark gate passes. |
| Cloud ingestion built on unvalidated demand, burning weeks on X-Ray/Datadog adapter pain | V | M | Gate Stage 3 behind explicit demand validation (see Open Questions). Build Tempo + local-file first — cheap and open. |
| 9–10 weeks is aggressive for a first real Rust systems project; the learning curve eats the schedule | F | **H** | Milestone-gated build (the slab pattern) with the week-3 slip rule. Rust learning is priority 3 — take the boring idiomatic path, not the instructive one, and reach for a crate rather than hand-rolling. Ship a rough Stage 1 over a polished nothing. |
| Author has no existing distribution, so launch lands silently | B | M | Build launch surface in parallel: OTel community Slack presence, a written benchmark post comparing against Jaeger on the exact issue numbers, Show HN timed to the benchmark result. |

**Validation status:**

| Risk Type | Question | Status |
|---|---|---|
| **Value** | Will developers want this? | ✅ Partially — OTel's own survey names the need; incumbents prove a market exists |
| **Usability** | Can users figure it out? | ⬜ Unvalidated — no prototype, no beta users |
| **Feasibility** | Can it be built, and fast enough? | ⬜ **The critical unknown.** Rust ecosystem is adequate; the performance claim is unproven |
| **Viability** | Does it work as a credibility asset? | ⬜ Depends entirely on whether the performance claim holds and is demonstrated publicly |

---

## Open Questions

| Question | Assumption | How to Validate | Timeline |
|---|---|---|---|
| Can the **render path** hold a 40k-span trace interactively in under 3s? (The store demonstrably can — this was never the storage question it looked like.) | Yes, with virtualization; canvas as the fallback | M1 browser spike: real payload from the real server, virtualized rows, `performance.now()` on fetch / parse / first-paint / keystroke-repaint. Compare against Jaeger all-in-one and otel-desktop-viewer on identical input, synthetic **and** the OTel demo app. **Method guards (added 2026-09-13):** read each trace back and assert its span count before timing anything — Jaeger truncates silently via collector queue size and `max-traces`, so an unverified push can time an 11k trace you believe is 40k. `performance.now()` applies to OUR viewer only; incumbent keystroke latency is measured coarsely in DevTools (10 runs, median + p90), which a 2x relative gate survives. Any *single* incumbent inside the good bands kills the performance headline. | **Week 2. Before M2 picks a framework.** |
| ~~Arrow-in-memory or DuckDB-for-everything?~~ | **DECIDED 2026-09-13:** neither. Trace-partitioned native struct-of-arrays, hand-rolled predicate scan, no embedded query engine. Persistence is length-delimited OTLP protobuf; Parquet is a v1.1 export feature. | Settled by counting the actual v1 queries: a point lookup and a sub-millisecond scan over ≤40k rows. See "Decided Against". | **Closed.** Do not reopen without a cross-trace-search requirement. |
| Is Arrow worth carrying in v1 purely for the columnar/Parquet narrative? | No — v1.1 export only | Owner's call, and it is a narrative choice rather than a performance one. If taken, it must not pull DataFusion back in with it. | Before v1.1 |
| `--max-memory` default: 512 MiB, 1 GiB, or 2 GiB? | 1 GiB | Pin after M1 measures bytes/span | Week 2 |
| UI framework for M2: none (Vite + TS), or React for velocity? | None — 40k virtualized rows is where a reconciler becomes the adversary | Let the M1 browser spike decide empirically. Either way the waterfall is hand-managed or canvas. | Week 3 |
| Do developers want production traces pulled into a local viewer? | ⚠️ Yes, but evidence is indirect | Ship Stage 1, then ask directly: r/rust, r/devops, OTel Slack, Show HN thread. Count unprompted requests before building any adapter | After Stage 1 launch |
| Is trace diffing a headline feature or a nice-to-have? | Headline — it's the clearest gap | Include a diff mockup in the beta-user conversations; see if it's what they react to | Pre-Stage-2 |
| Logs in v1.1, or never? | v1.1 with trace correlation | Track how often beta users ask for log→trace jump | During Stage 1 beta |
| Web UI or TUI as the primary surface? | Web UI | Settled by the feature set (diffing and deep-zoom need pixels). Revisit only if beta users push back | Decided — revisit at Stage 2 |
| What is this thing called? | `tracescope` is a placeholder | Check crates.io, GitHub, and npm for collisions; pick before the first public commit | **Before first commit** |
| Apache-2.0 or MIT? | Apache-2.0 (patent grant, matches OTel ecosystem) | Decide and commit | Before first commit |

---

## Milestone Plan

Gated, in the pattern already used on slab — each gate is pass/fail, and nothing downstream starts until it passes.

**Stage 1 — v1 (the wedge): 9–10 weeks** (re-costed 2026-09-13; see T14 — the table below still needs the expansions distributed across milestones)

| Week | Milestone | Exit criteria |
|---|---|---|
| 1 | **M0 — Ingest + store + diagnostics data** | Span schema implemented (columns, CSR attributes, insert-time parent resolution); `convert` with the full rejection taxonomy and its typed-error contract; both transports including wrong-port protocol sniffing; memory-capped quietest-trace eviction; **`ReceiverStats` and the reject ring populated**; all of it provable via `--dump`, including span flags and rejects |
| 2 | **M1 — GO/NO-GO gate** | Synthetic 10k/40k/100k generator **and the OpenTelemetry demo app** as a real-telemetry source; backend benchmark; **throwaway browser spike** (one static HTML file, virtualized absolutely-positioned rows, one filter input, `performance.now()` instrumentation); measured against Jaeger all-in-one and otel-desktop-viewer on identical input. **GO/NO-GO on the browser-measured end-to-end number, not a backend microbenchmark.** **Method guards:** read every trace back and assert its span count BEFORE timing (Jaeger truncates silently via collector queue size + `max-traces`). `performance.now()` instruments OUR viewer only — incumbent keystroke latency is DevTools-measured, 10 runs, median + p90. Any *single* incumbent inside the good bands kills the performance headline. |
| 3–5 | **M2 — Web UI** | Virtualized waterfall, collapse/expand, span detail (attributes fetched on click), attribute search and filter — all holding the M1-validated budgets |
| 6 | **M3 — Receiver debug panel** | A *view* over data M0 already produces: what arrived, from which endpoint and protocol, what was rejected and why, which spans have broken parent links, eviction and late-arrival counters. Budgeted at 1–2 days, not a feature build. Remainder of the week: beta users, trace-list polish. |
| 7–8 | **Ship** | Single-binary releases (macOS arm64, Linux x86_64), README carrying the benchmark numbers and the specific Jaeger issue numbers addressed, benchmark write-up published with the generator, harness, and raw numbers in-repo |

**Why the Receiver data moved to M0.** It is a struct of counters plus a ring buffer, produced as a byproduct of ingest — there is no reason to defer it, and deferring it was creating two risks at once: M3 landing days before launch with no bug-fix buffer, and the M1 gate having no fallback differentiator if the performance thesis failed. Both close by moving the data (not the panel) to week 1.

**Slip rule:** if the M1 gate hasn't cleared by end of week 3, cut the M3 *panel* and ship M0–M2 only. The diagnostics themselves survive regardless, via `--dump` and `/api/receiver`. The window is worth more than the feature set.

**M1 pivot plan (defined now, not improvised at the gate):**
- *Backend fast, browser slow* → renderer pivot: single `<canvas>`, draw rects, hit-test on click. Contained change, well-trodden, not a project pivot.
- *Payload- or parse-bound* → little-endian packed binary over `DataView`, still zero new dependencies.
- *Open-40k genuinely unachievable* → **thesis pivot, executed honestly.** The headline becomes "the only local viewer that tells you why nothing showed up," with persistence and diffing behind it. Survivable precisely because the Receiver diagnostics already exist at gate time. Publish the benchmark either way: a credible negative result is still a credibility asset, a debunked positive one is not.

**Stage 2 — v1.1 (depth)**
- On-disk persistence: length-delimited OTLP protobuf (versioned container), temp-then-rename, non-fatal on failure
- Parquet **export** behind a feature flag (this is where `arrow-rs` earns its dependency)
- Trace diffing / before-after comparison
- Logs with trace correlation (log → span jump)
- Load stored OTLP JSON and protobuf files, and objects from S3 — reuses `convert` verbatim

**Stage 3 — v2 (deferral confirmed; proceed only if validated)**
- Grafana Tempo pull first (open API, no quota)
- Then Datadog (surface the 300 req/hour limit in the UI), then X-Ray (surface the 5-ID batch limit)
- Skip Honeycomb pending Enterprise API access

---

## Before Finalizing

- [ ] Re-verify competitor star counts, latest releases, and maintenance status — the research snapshot will drift
- [ ] Check whether jaeger-ui has shipped large-trace virtualization since the research date
- [ ] Check whether otel-desktop-viewer has added diffing or cloud pull
- [ ] Confirm no naming collision on crates.io, GitHub, and npm — **now blocking**: the name goes into the crate name, binary name, config env prefix, and repo URL
- [ ] Confirm the 40k-span failure in Jaeger still reproduces on current Jaeger v2 — the benchmark story depends on it
- [ ] Set `panic = "unwind"` in the release profile. Do not set `panic = "abort"` for binary size, or the `catch_unwind` guarantee in the ingest path silently stops working.

## Sign-off

| Role | Name | Approved |
|---|---|---|
| Product | Karthik | ⬜ |
| Engineering | Karthik | ⬜ |
| Design | — (unstaffed; see Usability risk) | ⬜ |
