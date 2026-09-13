# Spec 02 — M1: the GO/NO-GO gate

**Tasks:** none directly. Source: `docs/prd.md` §Milestone Plan, M1 row (exit criteria), §Open Questions →
render-path row (the question it closes), and §Milestone Plan → M1 pivot plan.
**Owns:** `bench/generator.*`, `spike/`
**Preconditions:** spec 01 complete (M0 must be able to serve a real payload from a real server).
**Week:** 2. **Before M2 picks a UI framework.**

---

## M1 is not T2. Read this before starting.

Both measure trace-opening latency and the two are easy to conflate.

| | T2 (spec 00) | M1 (this spec) |
|---|---|---|
| When | Day 1–2, before any product code | Week 2, after M0 |
| Measures | **Incumbents only** | **Our viewer**, against incumbents on identical input |
| Decides | The launch *narrative* — does the performance story exist | Whether the *architecture* holds, and which renderer to use |
| Input | T3's throwaway producer | This spec's benchmark generator |
| Failing it means | No perf headline. Build continues unchanged. | A renderer or payload pivot (below). Still not a project pivot. |

T2's producer and this generator are **deliberately separate artifacts.** Sharing them would make
the day-0 spike wait on M0.

---

## Deliverables

### 1. Synthetic generator — `bench/generator.*`

Traces at **10k / 40k / 100k** spans. Realistic parent/child trees, not flat lists. Deterministic
(seeded), so a re-run compares against a previous run.

### 2. A real-telemetry control

The **OpenTelemetry demo app under load**, alongside the synthetic traces. Synthetic-only invites
"that's not realistic" at launch, and that objection lands whether or not it is fair.

### 3. Backend benchmark

Ingest throughput, spans/sec sustained, **both protocols concurrently**. This is the one axis that
can be measured without a browser.

### 4. Throwaway browser spike — `spike/`

**One static HTML file.** Virtualized absolutely-positioned rows, one filter input,
`performance.now()` instrumentation. Throwaway on purpose: its job is to measure the render path
before a framework is chosen, so building it *in* a framework would beg the question.

It measures the real payload from the real M0 server — not a fixture.

---

## The three axes, all required

A backend-only gate gates nothing: roughly **90% of the latency budget is browser-side**. This is
why M1 includes a browser spike at all.

1. **Ingest throughput** — spans/sec sustained, both protocols at once.
2. **Trace-open end-to-end** — backend fetch + browser parse + first paint.
3. **Filter latency per keystroke**, measured in the browser, including the localhost `/filter`
   round-trip.

**The GO/NO-GO is called on the browser-measured end-to-end number, not on a backend
microbenchmark.**

### Targets

| Axis | Target |
|---|---|
| Open a 10k-span trace | <1s to first paint |
| Open a 40k-span trace | <3s, no hang (this is the jaeger-ui #562 failure) |
| Filter keystroke on an open 40k trace | <100ms end-to-end, measured in the browser |
| Collapse/expand subtrees on 40k spans | No jank, and **no trace re-fetch** |

---

## Method guards — the same four as T2, restated because they apply here too

1. **Read every trace back and assert its span count BEFORE timing anything.** Jaeger truncates
   silently via collector queue size and `max-traces`, so an unverified push can have you timing an
   11k-span trace you believe is 40k.
2. `performance.now()` instruments **our viewer only.** Incumbent keystroke latency is
   DevTools-measured, 10 runs, median + p90.
3. **Any *single* incumbent inside the good bands kills the performance headline.** Not an average.
4. **Report the spread.** Variance above 30% means the harness is not good enough to call a
   threshold decision.

Comparison targets: **Jaeger all-in-one and otel-desktop-viewer** on identical input.

---

## Pivot plan — defined now, not improvised at the gate

| Failure shape | Pivot | Scope of the change |
|---|---|---|
| Backend fast, browser slow | Single `<canvas>`, draw rects, hit-test on click | Contained renderer change, well-trodden. **Not a project pivot.** |
| Payload- or parse-bound | Little-endian packed binary over `DataView` | Zero new dependencies. The escape hatch spec 01 §6 names. |
| Open-40k genuinely unachievable | **Thesis pivot, executed honestly.** Headline becomes "the only local viewer that tells you why nothing showed up," with persistence and diffing behind it | Survivable precisely because the Receiver diagnostics already exist at gate time (spec 01 §7) |

Publish the benchmark either way. **A credible negative result is still a credibility asset; a
debunked positive one is not.**

---

## What M1 pins for downstream specs

M1 is not only a gate; three later decisions wait on its numbers.

| Output | Unblocks |
|---|---|
| Measured **bytes/span** | STOP #5 — the `--max-memory` default (512 MiB / 1 GiB / 2 GiB) |
| Whether a reconciler is affordable at 40k rows | STOP #7 — the UI framework, and therefore all of spec 05 |
| Whether the designed baseline meets the budgets | Whether canvas / packed-binary tuning hours are funded at all |

Report all three explicitly at the gate. They are the reason the gate is scheduled *before* M2
picks a framework.

---

## The slip rule

If the M1 gate has not cleared by **end of week 3**, cut the M3 *panel* and ship M0–M2 only. The
diagnostics themselves survive regardless, via `--dump` and `/api/receiver`. **The window is worth
more than the feature set.**

Note this rule is currently stated against a milestone table that has not been re-costed for the
scope expansions — that is T14 (spec 07), and until it lands the slip rule's week numbers are
approximate.

---

## Acceptance

Cite the test plan's "Critical Paths" → M1 GO/NO-GO gate rows and the method-guard row.

| # | Check |
|---|---|
| 1 | All three axes measured. A result missing any axis is not a gate result. |
| 2 | Span counts read back and asserted before any timing, for every trace and every subject. |
| 3 | Both synthetic **and** OTel demo app inputs used. |
| 4 | Incumbent numbers: DevTools, 10 runs, median and p90, spread reported. |
| 5 | Machine specs published alongside the numbers. |
| 6 | bytes/span reported, so STOP #5 can be closed. |
| 7 | A framework recommendation reported with the evidence behind it, so STOP #7 can be closed. |
| 8 | If any target was missed, the corresponding pivot from the table above was chosen — not a new one invented at the gate. |

## Out of scope

Building the production waterfall — the spike is throwaway and must stay throwaway. Choosing the UI
framework (M1 produces the evidence; the decision is STOP #7). Any tuning work not funded by the
result. Re-running T2's incumbent-narrative verdict: that verdict is already recorded and M1 does
not revisit it.
