# Spec 03 — the instrumentation linter

**Tasks:** T5, T6, T7, T8, D1, D2, D13
**Owns:** `src/lint/`, `src/store/lint_view.rs`
**Preconditions:** spec 01 complete. **T1 complete** — it can delete rules and move thresholds.
**Registers CLI subcommands** (`lint`) described here and wired by spec 07.
**The `GET /api/lint` HTTP handler lives in `src/api/`, which spec 01 owns.** Same pattern as
`src/main.rs`/spec 07: this spec defines the response shape and the `lint::Finding` type, spec 01
wires the route against it. Do not add the handler to `src/lint/`.

---

## What this feature is for

One sentence, because every design decision below follows from it:

> Run `lint`, see findings, **fix the instrumentation, re-run the request, run `lint` again, read
> clean** — without restarting the process.

That loop is the product. If it does not close, the feature has failed even if every rule is
correct.

---

## What NOT to build — an earlier design was deleted, and its parts keep looking necessary

Lint is computed **on request** and holds **no long-lived state**. An earlier version of this
design accumulated findings live on the ingest path, and that version was deleted wholesale.
Thirteen sub-decisions existed only to support it. **None of them should be built:**

no accumulator · no epochs · no `lint reset` · no `since <time>` label · no span-name cap · no
attribute-key cap · no saturation latches · no per-source scopes (live / demo / file) · no
generation-checked evidence links inside lint · no writer-vs-handler feed point · no lint state for
eviction to prune · no per-row scope labels · `src/lint/accumulator.rs` **is not a module in this
design**

Two things to understand, so they are not reinvented under new names:

1. **"Never pruned" breaks the loop above.** A monotonic accumulator can never reach `clean` without
   killing the process. Any mechanism that carries findings forward across requests recreates that
   bug.
2. **The per-span cost was the reason it moved off ingest.** It was once claimed to be "O(1), two
   hash inserts." It is per-*attribute* — roughly 400–800k hash probes at 40k spans. On request it
   is one pass over resident traces, **paid by the person who typed `lint`.**

`lint` depends on `store`, never the reverse. **`ingest` does not know `lint` exists.**

---

## T6 — `store::LintView`

**Effort:** ~2h human / ~20min CC.

A read accessor over the store, in `src/store/lint_view.rs`.

This task was **inverted** from an earlier "do not build — it may have no consumer" decision. It is
now the only way lint reads the store, so it is required. It has exactly one consumer and that
consumer is the feature.

Contract: take the read lock, **copy out** what the rules need, **release before evaluating.** A
rule must never run while holding the lock — a slow or panicking rule would then block ingest.

It is a **new file under `src/store/`**, not an edit to the writer. That is deliberate: it keeps
this spec's lane from colliding with spec 01's.

---

## T5 + D1 — scope: the decision that makes the loop close

**T5:** ~3d human / ~4h CC. **D1:** ~2h human / ~20min CC.

| Invocation | Scope |
|---|---|
| `lint` | **The newest resident trace.** The default. |
| `lint --all` | Every resident trace. Explicit. |
| `lint <file.otlp>` | One capture, via the Stage 2 load path (reuses `ingest::convert` verbatim) |

**Why the newest trace is the default.** On-request lint reads whatever is resident. After you fix
your instrumentation, the old bad traces are *still in the store*, so an all-resident default keeps
reporting the old findings and `clean` is unreachable — the same failure as the deleted accumulator,
arrived at from a different direction. Defaulting to the newest trace closes the loop with **no new
state and nothing destructive.**

Every run states its input: how many traces it examined, or the file path. Under-reporting is
therefore visible by construction rather than silent.

---

## D2 — every rule declares its scope, and skipped rules are named

**Effort:** ~2h human / ~20min CC.

The immediate consequence of D1: a rule that needs many traces **cannot fire** at newest-trace
scope. Left implicit, it reads as a pass. That is a silent false negative in a correctness tool.

Each rule declares `Scope::PerTrace` or `Scope::CrossTrace`. The default run evaluates the
per-trace rules and **names the skipped ones, with the command that runs them.**

> **A skipped rule must never render as a pass.** This is the acceptance criterion for D2.

---

## The five rules

**STOP #6 resolved 2026-09-13**, via T1's measurement against 11,289 real spans from the OTel demo
app (`scripts/spike-harness/t1-findings-20260913.md`). Six were specified; one is cut, one predicate
is fixed, four are confirmed as originally written. Implement each rule as an independent unit in a
registry array, so cutting or changing one is a file deletion or a predicate edit — never a
refactor.

| # | Rule | Predicate | Scope | Tier |
|---|---|---|---|---|
| 1 | High-cardinality span names | distinct span names **≥ 200**. Reports the top 5 offenders with example names. | **Cross-trace** | WARN |
| 2 | Missing `service.name` | Any span whose resource lacks it. Reports count and percent; **names the endpoint and protocol it arrived on.** | Per-trace | ERROR |
| 3 | Broken context propagation | Orphan spans grouped by (child service, unresolved parent span id). Fires when a service has **>0 orphans** whose parent was never seen inside the retention window AND the trace contains spans from **2+ services**. | Per-trace | ERROR |
| 4 | Attribute type conflict | Same attribute key observed with **2+ different value types**. Reports the key, both types, and counts. | **Cross-trace** | WARN |
| 5 | Long-duration leaf span | Zero children AND duration **> 1s** AND **> 50% of its parent's duration** AND the span is **not** a streaming/long-poll RPC (no `rpc.*`/`messaging.*` attribute indicating a server-streaming or subscription call). Suggests missing instrumentation inside it. | Per-trace | INFO |

Rule 3's predicate is **not evaluable at append time**, because out-of-order spans are normal in
OTLP. It is evaluable on request, which is one more reason the on-request model is the right one.

Rule 1's specification-MUST citation ("Instrumentation MUST NOT default to using URI path as a
`{target}`") and its downstream-harm framing (**metric generators turn `span.name` into a label, so
high cardinality causes a metrics spike**) still apply — only the predicate's shape changed.

### T1's measurement (2026-09-13) — what changed and why

Full write-up: `scripts/spike-harness/t1-findings-20260913.md`.

1. **Rule 1 — ratio replaced with an absolute count.** As specified (>50 distinct AND >30% of
   total), the predicate never fired on real data: the demo app's 17 services produced 87 distinct
   span names across 11,289 spans — a 0.77% ratio, nowhere near the 30% bar. Distinct names grow
   sub-linearly (bounded operation vocabulary per service) while total spans grow with traffic,
   exactly as flagged before T1 ran. Fixed to an absolute **≥200** — clearly above this app's clean
   baseline of 87, though the exact number still wants a genuinely bad project to calibrate a true
   positive, not just a clean negative.
2. **Rule 5 — added a streaming-RPC exclusion.** As specified, it fired 24 times, and all 24 were
   the identical false-positive shape: a long-lived gRPC streaming subscription
   (`flagd.evaluation.v1.Service/EventStream`), which is supposed to run long — that is not a
   missing-instrumentation bug. Excluded via a streaming/long-poll signal on the span's attributes.
3. **Rule 6 — cut for v1.** As specified, it fired on ~12,025 of 11,289+ spans (a span can carry
   more than one deprecated key): the **OpenTelemetry project's own reference demo app** is still
   instrumented with pre-1.23 HTTP semconv names (`http.method`, `http.status_code`, `http.url`,
   etc.). If the ecosystem's flagship example hasn't migrated, this rule would flag the majority of
   real-world HTTP-instrumented services on their first run — the "fires on nearly everything, and
   noise is worse than silence" case T1 exists to catch. A narrower version (excluding the three
   keys responsible for ~75% of hits, keeping only genuinely rare deprecated keys) is recorded in
   `TODOS.md` as a possible v1.1 revisit, not built now.
4. **Rules 2, 3, 4 — confirmed.** Rules 3 and 4 fired with real, specific, actionable findings
   (cross-service orphan spans; attribute type drift across the demo's polyglot SDKs) and ship
   unchanged. Rule 2 fired zero times — expected on a well-instrumented reference app, which can
   only produce a true negative for a rule about *missing* instrumentation, not disprove it. Kept as
   specified (ERROR tier, cheap, catches a real and common first-instrumentation mistake this
   dataset simply didn't have).

---

## T7 — one `lint::Finding` type

**Effort:** ~2h human / ~20min CC.

One type owns: rule id, severity, **count**, `examined` (traces or file path), samples, span link,
and **the headline sentence**.

The terminal formatter and the JSON API are **thin formatters over this type.** Neither composes
its own wording. If the headline lives in two places, the two surfaces drift, and this was recorded
as a finding rather than a style preference.

Samples: `[(trace_id, span_idx, generation); ≤3]`. Since findings are computed from resident
traces, every sample is a live reference at compute time — no generation check is needed *inside
lint*. **The `generation` field exists for later, not for now (eng review, 2026-09-13):** the JSON
response travels to the browser and may sit there before the reader clicks a sample — long enough
for the trace to be evicted and re-arrive (`RESURRECTED`, spec 01), reusing the same `trace_id` with
a bumped generation and reset span indices. Without `generation` in the sample itself, the client
has nothing to compare against spec 01's generation check when it later dereferences the link, and
would silently open the wrong span in the new trace. Include it in every sample tuple so the
existing dereference check (spec 01 §1) has something to compare.

---

## Display rules

Both surfaces, identically:

- **≤6 rows per run.** One collapsed line per rule, with a count. This is a **display** choice
  evaluated at render time, not a stored cap — findings are recomputed per request so there is
  nothing to cap.
- **Samples ≤3, behind a disclosure.**
- **Group headings do not count as rows.**
- **A group with no firing rules is not rendered.** A group whose rules were all skipped is
  represented by the skipped line only — **never a heading with nothing under it.**

### Severity is carried by the group heading, not by a token

The headings name what the tier *means*, so the meaning survives greyscale, printing and
colour-blindness. Colour is reinforcement, never the signal.

| Tier | Heading, verbatim | Rules |
|---|---|---|
| ERROR | `BREAKS ANALYSIS` | 3, 2 |
| WARN | `COSTS YOU MONEY DOWNSTREAM` | 1, 4 |
| INFO | `ADVISORY` | 5 |

Without tiers, simultaneous findings read as flat noise and none get fixed.

---

## Output — default scope (newest trace)

Only the four per-trace rules can fire, so the cross-trace group is **absent, not empty.** Every
count is scoped to this one 412-span trace.

```
[Traces] [Lint] [Receiver]                    :4317 grpc · :4318 http
POST /checkout · 412 spans · 13:04:22 (newest trace)          Re-run

BREAKS ANALYSIS
| Broken context propagation — 3 services have orphan spans        47
| Spans missing service.name — arriving on :4318 http/protobuf   118
|   checkout.handler     4bf92f3577b34da6
|   db.query             a3ce929d0e0e4736
|   payment.authorize    00f067aa0ba902b7
|   115 more

ADVISORY
| Long-duration leaf spans over 1s with no children                   12

2 rules need more than one trace (span-name cardinality, attribute type
conflict) — run `tracescope lint --all`, 214 traces resident
```

## Output — `--all` scope

Three groups, all five rules, and both the scope line and the footer change. **This is the only view
in which the cross-trace rows exist.** (The design doc elides this block with `...`; it is expanded
here because implementation follows the spec, not the mockup.)

```
[Traces] [Lint] [Receiver]                    :4317 grpc · :4318 http
POST /checkout + 213 others · 40,118 spans · since 09:12       Re-run

BREAKS ANALYSIS
| Broken context propagation — 9 services have orphan spans     2,180
| Spans missing service.name — arriving on :4318 http/protobuf  9,442
|   checkout.handler     4bf92f3577b34da6
|   db.query             a3ce929d0e0e4736
|   payment.authorize    00f067aa0ba902b7
|   9,439 more

COSTS YOU MONEY DOWNSTREAM
| High-cardinality span names — 13,402 distinct across 40,118 spans  13,402
| Attribute type conflict — http.status_code seen as int and string      88

ADVISORY
| Long-duration leaf spans over 1s with no children                    214

all 5 rules evaluated · examined 214 traces
```

Note the cardinality number **satisfies rule 1's predicate**: 13,402 distinct is ≥200. Illustrative
numbers that do not satisfy their own predicate are how a mockup teaches the wrong behaviour — see
T1's measurement above.

## Terminal output

`lint` prints the same findings as the tab; both are thin formatters over `lint::Finding`. Piped to
a file: **identical content, no ANSI escape codes.** Review the two together or they drift.

### D13 — the running process says one thing, once

**Effort:** ~1h human / ~10min CC.

The server process prints what it receives, plus **one** instrumentation nudge the first time spans
arrive, then never again:

```
receiving from checkout on :4318 — 412 spans
  run `tracescope lint` to check the instrumentation (5 rules)
```

**Nothing is printed on a timer.** An earlier spec said "on startup and then throttled every ~5s
when findings change" — that described the deleted accumulator. Nothing recomputes on a timer, so
there is no cadence to throttle. Delete that spec wherever it appears.

---

## States

| Condition | Output |
|---|---|
| Store empty | **"no data yet"** — never "0 issues found". Zero findings because nothing was examined is not a pass, and saying `0 issues` claims a clean bill of health for data that does not exist. |
| Findings | ≤6 rows, each with count + what it examined + ≤3 samples |
| No findings | **"clean across N traces"** — names its input |
| A rule panics | **"1 rule failed"**, the other rules still shown |
| Cross-trace rules not run | The skipped line, naming them and the command |

### Rule panics are contained

Each rule runs under `catch_unwind`; a panicking rule is **skipped and counted**, and the run
reports `1 rule failed`. One rule encountering odd data must not take down the run or the process.
This is also why evaluation happens after the read lock is released.

Rendered as its own line after the last group and before the skipped-rules footer (if any), not
inside a group:

```
ADVISORY
| Long-duration leaf spans over 1s with no children                   12

⚠ 1 rule failed: attribute type conflict (internal error) — other findings unaffected
```

## API

`GET /api/lint` returns the findings for the same default scope as the CLI, with the scope, the
`examined` count, and the skipped-rule list as structured fields — not as a pre-rendered sentence.
The route is **`/lint`** served from the Lint tab, backed by `GET /api/lint`. It is **not**
`/instrumentation`, a name that appeared once and is retired.

---

## Acceptance

Cite the test plan: the **fix-and-verify loop** case (the single most important test in the file),
the `lint --all` case, the skipped-rules case, `lint <file.otlp>` parity, the zero-spans case, the
attribute-type-conflict case, the piped-output case, and the one-time-nudge case.

Spec-specific additions:

| # | Check |
|---|---|
| 1 | **Fix-and-verify, no restart.** `lint` → findings; fix instrumentation; re-run the request; `lint` → **clean**, with the old bad traces still resident. |
| 2 | `lint --all` on that same store **still reports the old findings**, and says how many traces it examined. Both readings are correct and each names its input. |
| 3 | The default run names both skipped rules and prints the command. Grep the output for the rule names. |
| 3b | **(eng review, 2026-09-13)** A trace with zero per-trace findings still prints the skipped-rules line alongside "clean" — clean-at-this-scope and rules-not-run-at-this-scope are independent facts and both render together, never one suppressing the other. |
| 4 | `--all` evaluates all six and says so. |
| 5 | Two `lint` calls with no traffic between them return **identical** output. |
| 6 | A deliberately panicking rule yields "1 rule failed" with the other findings intact, and the process survives. |
| 7 | Terminal output piped to a file has no ANSI codes and matches the tab's content. |
| 8 | The nudge prints exactly once, on the first batch. Nothing further prints about instrumentation. |
| 9 | Empty store says "no data yet", not "0 issues found". |
| 10 | `lint examples/checkout.otlp` gives the same findings as linting that capture live. |
| 11 | **T8:** each surviving rule has one trace that trips it and one that does not. |
| 12 | Grep the tree: no `accumulator`, no `epoch`, no `reset` subcommand, no `since` label. |

## Out of scope

Lint threshold config file — **hardcoded constants in v1; the first complaint is the signal.** CI
assert mode (`TODOS.md`, it is the linter with an exit code — the natural v1.1 headline). Watch mode
and diff. Any accumulator-era mechanism from the deleted list above. Ranking findings beyond the
three tiers.

## STOP items live here

**#6 — resolved 2026-09-13.** Five rules survive (rule 6 cut, rule 1's threshold changed to an
absolute count, rule 5 gained a streaming-RPC exclusion). See "T1's measurement" above and
`scripts/spike-harness/t1-findings-20260913.md`.
