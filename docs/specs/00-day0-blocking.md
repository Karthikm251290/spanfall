# Spec 00 — Day 0, blocking

**Tasks:** T0 (name), T1 (rule validation), T3 (producer), T2 (spike)
**Owns:** `scripts/producer.*`, `scripts/spike-harness/`
(**not** `bench/` — spec 02 owns that tree; the two specs run at different times, possibly in
different worktrees, and a shared parent directory is how they collide)
**Preconditions:** none. This is the first work. **No product code is written in this spec.**
**Blocks:** T0 blocks the public repo. T1 blocks spec 03. T2 gates the launch narrative.

Why day 0 and not "before the first commit": the spike ships a **public benchmark repo**, so the
name is needed before anything is published, and T1 uses the **OTel Collector's file exporter**
rather than any tracescope code, so it has no dependency on M0 existing.

---

## T0 — Decide the name

**Effort:** ~1h. **STOP item #1 and #3.**

The name lands in: crate name, binary name, env-var prefix, repo URL, benchmark repo URL, and
every document. `tracescope` is a placeholder throughout these specs.

Deliverables:
1. Collision check on **crates.io, GitHub, and npm** (`docs/prd.md` §Before Finalizing). npm matters because a
   name squatted there poisons search results even for a Rust tool.
2. A license decision. Apache-2.0 is the assumption, for the patent grant and OTel-ecosystem
   consistency. Not yet decided — see STOP #3.
3. The chosen name written into `Cargo.toml` in exactly one place, with the binary name derived
   from it, so no source file hardcodes it.

**Do not** pick the name yourself. Present the collision-check results and let the owner choose.

---

## T1 — Validate the six rules before building the engine

**Effort:** ~half day. **Gates spec 03. STOP item #6.**

This inverts the CEO plan's original order, which scheduled validation *after* the rules existed.
An afternoon here precedes ~1.5 weeks of engine work, and its outcome can delete rules.

### Step 1 — verify the premise of the method itself

The task assumes **2–3 already-instrumented real projects exist** to point this at. That
assumption was flagged as unchecked by the review that created the task. **Verify it first.** If
they do not exist, "half a day" becomes "instrument three applications first," which is a
different task with a different cost — report that rather than absorbing it.

The OpenTelemetry demo app is the one guaranteed input.

### Step 2 — collect the data with no tracescope code

Use the **OTel Collector's file exporter**. Point each instrumented app at a Collector configured
to write OTLP to a file, then analyse the files offline. No `--dump`, no M0, no product code.

What to extract per project: every distinct span name with its count; every attribute key with the
set of value types observed; resource attributes present and absent per service; parent/child span
id pairs with unresolved parents; leaf-span durations with parent durations.

### Step 3 — for each of the six rules, answer two questions by hand

Would it fire on this data, and would the firing be **useful** to the developer who owns that code?
Useful means specific and actionable, not merely true.

### Decision rule — commit to it before looking at the output

| Outcome | Meaning | Action |
|---|---|---|
| Fires nothing on real code | Rules are too conservative, or the premise is wrong | Do not build a launch narrative on the linter. Report before proceeding. |
| Fires on nearly everything | Thresholds too loose, or findings need ranking before they are useful | Tighten thresholds and re-run this step. Noise is worse than silence here. |
| Fires a handful of true, specific, actionable findings per project | The feature is real | Proceed to spec 03 with the surviving rule set. |

### Deliverables

1. A table: rule × project → fired / did not fire / would have been noise.
2. **The surviving rule set**, explicitly listing which rules were cut and why.
3. **Pinned thresholds**, with the observed number that justifies each one. The thresholds in
   spec 03 are reasoned, not measured; this is where they become measured.
4. A note on rule 6 (deprecated semconv) specifically: confirm whether it false-positives on a
   project pinned to an older SDK. That is the documented objection to it.

The two rules to scrutinise hardest, because they were challenged by name: **rule 2** (missing
`service.name`) may duplicate what the Receiver panel already shows, and **rule 6** is a
maintenance-bearing table that rots as OTel evolves.

---

## T3 — Throwaway trace producer

**Effort:** ~2h.

A ~50-line script that emits traces at a controllable span count to an OTLP endpoint.

**This is explicitly NOT M1's benchmark generator.** Keeping them separate is what stops the spike
from waiting on M1. Write it as a throwaway; M1's generator (spec 02) is a different artifact with
different requirements, and duplication between them is correct here.

Requirements: emit a parameterised number of spans in one trace; produce a realistic parent/child
tree rather than a flat list, because a flat list does not exercise the incumbents' tree
expansion; target both 4317 and 4318. Any language.

---

## T2 — The verification spike

**Effort:** ~2.25 days human. **Gates the launch narrative, not the build.**

### What is being verified — one hypothesis, stated precisely

> On a current version of each incumbent, opening and interactively filtering a 40,000-span trace
> is slow enough that a well-built alternative is visibly better to a developer.

**Not** being verified: market size, whether people want a local viewer (the OTel DevEx survey
already answers that), or whether Rust is faster than Go — irrelevant, because the bottleneck is
the browser.

### Step 0 — the early exit. Run this first.

Jaeger's own release material claims a virtual viewport handling traces up to 50,000 spans with
testing reported at 80,000, while jaeger-ui #562 (40k traces do not load) remains open. Both
cannot be fully true and nobody has checked which describes reality today.

**Try to reproduce Jaeger's claim before building any harness.** If a current Jaeger v2 opens an
80k-span trace smoothly, the performance thesis is falsified on day 1 by the incumbent's own
benchmark, the rest of the spike is unnecessary, and you go straight to THESIS DEAD. This is the
cheapest available kill shot, which is why it runs first.

### Subjects

Jaeger v2 all-in-one, and otel-desktop-viewer 0.5.0.

**Record exact versions and test dates.** otel-desktop-viewer shipped 0.5.0 in late August 2026
and is actively developed, so a 0.6 can land mid-spike. Re-check both versions before publishing
anything.

### Inputs

Synthetic traces at **10k / 40k / 100k** spans via T3's producer, plus the OpenTelemetry demo app
under load as a real-telemetry control. Synthetic-only invites "that's not realistic" at launch.

### Method — four guards, all mandatory

These four exist because the first version of this spike was methodologically unsound in four
specific ways.

1. **Read every trace back and assert its span count BEFORE timing anything.** Jaeger truncates
   silently via collector queue size and `max-traces`. An unverified push can have you timing an
   11k-span trace you believe is 40k, which invalidates the entire result.
2. **Incumbent latency is measured coarsely in DevTools** — 10 runs, report median **and** p90.
   `performance.now()` cannot be injected into an incumbent's app. A 2x relative gate survives
   coarse measurement; a 10% one would not.
3. **Any *single* incumbent inside the good bands kills the performance headline.** Not an average
   across subjects. One fast incumbent is a sufficient counterexample to "they all collapse."
4. **Report the spread.** If run-to-run variance exceeds 30%, the measurement is not usable for a
   threshold decision and needs a better harness before the gate can be called.

Measured, in a real browser: time from trace-open click to interactive first paint; latency per
filter/search keystroke; peak browser memory. Warm cache, one machine, machine specs published.

### Pre-registered decision rule — commit before seeing numbers

Deciding the threshold after seeing data means rationalising whatever comes back.

The thresholds derive from two things. *Perceptual boundaries* (Nielsen / Card): ~100ms reads as
instantaneous, ~1s preserves flow of thought, beyond ~10s attention is lost. *A relative gap
requirement*: the question is not "is the incumbent slow" but "would a developer notice." A
sub-2x improvement inside the same perceptual band is not a differentiator anyone can feel. A
claim needs **both** a ≥2x gap **and** a crossed perceptual boundary.

| Spike result at 40k spans | Verdict | Action |
|---|---|---|
| Incumbent crosses a perceptual boundary badly: **>2s to open** (flow broken) or **>300ms per keystroke** (not live), or fails to load | **THESIS HOLDS** | Performance leads the launch; publish the benchmark post as the headline. Fund the extra render-path tuning. |
| Incumbent already sits inside the good bands: opens **<1.2s** and filters **<120ms** | **THESIS DEAD** | Performance is not a story. Launch on diagnostics + linter. Publish the benchmark anyway, honestly: "the incumbents are fine now, here is what is still broken." |
| Anything between, **or any result whose spread straddles a boundary** | **INCONCLUSIVE** | Treat as thesis-dead for positioning. Diagnostics and linter lead; performance becomes a supporting claim, never a headline. No perf-headline post. |

Straddling counts as inconclusive deliberately: with real measurement variance, the honest move on
an ambiguous number is the conservative branch, not the flattering one.

Note that the original THESIS HOLDS row contained a clause "and our designed baseline can plausibly
land ≥2x better" — **that clause was struck**, because "plausibly" is not falsifiable on observed
numbers. All three rows now decide on measurements alone.

### Fairness disclosure — stated in the write-up, not hidden

Jaeger all-in-one is a collector plus storage plus UI, not a single local binary. The comparison
is of the **developer-visible experience**, which is the honest framing and the one that survives
scrutiny. Do not claim a like-for-like architectural benchmark.

### Deliverable — the same either way

A public benchmark repo (producer + harness + raw numbers) and a write-up. **This is a credibility
artifact independent of the outcome.** A credible negative result is still an asset; a debunked
positive one is not.

### What the verdict does and does not change

The product is the same in every branch. It is the same OTLP ingest, the same trace-partitioned
store, the same waterfall, the same linter. What changes across branches: the launch narrative,
whether the perf-headline post ships, and whether the extra tuning hours (canvas renderer, packed
binary payload) are funded. **No branch of that table triggers a rewrite or discards work** — the
store design was chosen because it is simpler than the alternative it replaced.

---

## Acceptance

| # | Check |
|---|---|
| 1 | Collision check for the name covers crates.io, GitHub **and** npm; results presented, owner chose. |
| 2 | The "2–3 real instrumented projects" assumption was verified before T1 started, and the finding reported either way. |
| 3 | T1 produced a rule × project table, a surviving rule set with cut reasons, and a justifying number per threshold. |
| 4 | Step 0 (Jaeger's own 80k claim) was attempted before any harness was built. |
| 5 | Exact versions and test dates recorded for both subjects. |
| 6 | Span count read back and asserted for every trace, before any timing. |
| 7 | Incumbent numbers are 10 runs, median and p90, from DevTools. Spread reported. |
| 8 | The verdict was read off the pre-registered table without amending it. |
| 9 | Benchmark repo published with producer, harness and raw numbers, whatever the verdict. |

## Out of scope

No product code. No `Cargo.toml` beyond what T0 needs. No M1 benchmark generator (that is spec 02;
T3's producer is deliberately a throwaway). No render-path tuning — canvas and packed-binary work
is funded only by a THESIS HOLDS result, and is specified in spec 02's pivot plan.

## STOP items live here

**#1 name**, **#3 license**, and **#6 rule set** — all resolved 2026-09-13. Name: `spanfall`,
collision-checked clean. License: Apache-2.0. Rule set: five rules survive (see spec 03's "T1's
measurement" section and `scripts/spike-harness/t1-findings-20260913.md`).
