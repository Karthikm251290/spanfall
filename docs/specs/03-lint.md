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

## The six rules

**Thresholds below are reasoned, not measured. T1 replaces them with measured values and may delete
rules outright** (STOP #6). Implement each rule as an independent unit in a registry array, so
cutting one is a file deletion plus one line — never a refactor.

| # | Rule | Predicate | Scope | Tier |
|---|---|---|---|---|
| 1 | High-cardinality span names | distinct span names **> 50** AND distinct **> 30% of total spans**. Reports the top 5 offenders with example names. | **Cross-trace** | WARN |
| 2 | Missing `service.name` | Any span whose resource lacks it. Reports count and percent; **names the endpoint and protocol it arrived on.** | Per-trace | ERROR |
| 3 | Broken context propagation | Orphan spans grouped by (child service, unresolved parent span id). Fires when a service has **>0 orphans** whose parent was never seen inside the retention window AND the trace contains spans from **2+ services**. | Per-trace | ERROR |
| 4 | Attribute type conflict | Same attribute key observed with **2+ different value types**. Reports the key, both types, and counts. | **Cross-trace** | WARN |
| 5 | Long-duration leaf span | Zero children AND duration **> 1s** AND **> 50% of its parent's duration**. Suggests missing instrumentation inside it. | Per-trace | INFO |
| 6 | Deprecated semantic conventions | Attribute key matches a deprecation table **pinned to a stated OTel semconv version.** Pin it in the repo; bump deliberately. | Per-trace | INFO |

Rule 3's predicate is **not evaluable at append time**, because out-of-order spans are normal in
OTLP. It is evaluable on request, which is one more reason the on-request model is the right one.

Rule 1 can cite a specification MUST rather than a heuristic — "Instrumentation MUST NOT default to
using URI path as a `{target}`" — and its warning text should name the real downstream harm:
**metric generators turn `span.name` into a label, so high cardinality causes a metrics spike.**
That is what makes the finding actionable instead of merely true.

### Two things for T1 to check, flagged rather than decided

1. **Rules 2 and 6 were challenged by name.** Rule 2 may duplicate what the Receiver panel already
   shows; rule 6 is a maintenance-bearing table that false-positives on a project pinned to an older
   SDK. The recorded position is "six rules is probably three."
2. **Rule 1's ratio threshold may be unreachable at `--all` scope.** Distinct span names grow
   sub-linearly as traces accumulate while total spans grow linearly, so `distinct / total` *falls*
   as more traces arrive. A 30%-of-total threshold is much harder to cross over 214 traces than over
   one — and rule 1 is tagged cross-trace, so `--all` is the **only** scope it ever runs at.
   **Measure this in T1 before implementing the threshold as written.** A count-based or per-service
   predicate may be the correct shape. Do not silently change it here; report it.

---

## T7 — one `lint::Finding` type

**Effort:** ~2h human / ~20min CC.

One type owns: rule id, severity, **count**, `examined` (traces or file path), samples, span link,
and **the headline sentence**.

The terminal formatter and the JSON API are **thin formatters over this type.** Neither composes
its own wording. If the headline lives in two places, the two surfaces drift, and this was recorded
as a finding rather than a style preference.

Samples: `[(trace_id, span_idx); ≤3]`. Since findings are computed from resident traces, every
sample is a live reference — no generation check is needed *inside lint*. (The store's generation
counter from spec 01 still applies when a stored reference is dereferenced later.)

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
| INFO | `ADVISORY` | 5, 6 |

Without tiers, six simultaneous findings read as flat noise and none get fixed.

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
| Deprecated semantic conventions (semconv 1.27)                       6

2 rules need more than one trace (span-name cardinality, attribute type
conflict) — run `tracescope lint --all`, 214 traces resident
```

## Output — `--all` scope

Three groups, all six rules, and both the scope line and the footer change. **This is the only view
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
| Deprecated semantic conventions (semconv 1.27)                        61

all 6 rules evaluated · examined 214 traces
```

Note the cardinality numbers **satisfy rule 1's predicate as written**: 13,402 distinct is >50 and
is 33.4% of 40,118, clearing the 30% ratio. Illustrative numbers that do not satisfy their own
predicate are how a mockup teaches the wrong behaviour — see the T1 flag above.

## Terminal output

`lint` prints the same findings as the tab; both are thin formatters over `lint::Finding`. Piped to
a file: **identical content, no ANSI escape codes.** Review the two together or they drift.

### D13 — the running process says one thing, once

**Effort:** ~1h human / ~10min CC.

The server process prints what it receives, plus **one** instrumentation nudge the first time spans
arrive, then never again:

```
receiving from checkout on :4318 — 412 spans
  run `tracescope lint` to check the instrumentation (6 rules)
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

**#6** — how many rules survive T1, and whether rule 1's threshold shape changes. Do not implement
the six rules as written if T1 cut any of them or moved a threshold.
