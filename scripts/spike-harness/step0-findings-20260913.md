# T2 Step 0 — early-exit check, 2026-09-13

Per spec 00 T2 Step 0: reproduce Jaeger's own claimed 80k-span capability *before* building any
harness. If it holds, the performance thesis is falsified by the incumbent's own benchmark and the
rest of the spike is unnecessary.

## Subject

- Image: `jaegertracing/jaeger:2.20.0` (Jaeger **v2**, not the deprecated v1 `all-in-one` image —
  the `latest` tag on `jaegertracing/all-in-one` resolves to v1.76.0, EOL 2025-12-31; caught and
  corrected before testing).
- Storage: default in-memory.
- Test date: 2026-09-13.
- Machine: this session's environment (not a dedicated benchmark machine — Step 0 is a cheap
  pre-check, not the rigorous method T2's main body requires).

## Input

- `scripts/producer.py --spans 80000 --branching 6 --max-depth 20`, gRPC to `:4317`.
- Real parent/child tree, not flat — depth 20, single root `POST /db`.

## Method guard #1 — span count verified before timing

Queried Jaeger's `/api/traces` after ingest, before opening the UI:

```
traceID: 1077fc8985d8f0b72d0528a075065cd8  spanCount: 80000
```

Matches the 80,000 requested/emitted exactly.

## Measurement

Two navigations to `http://localhost:16686/trace/<id>`, read via the browser's own Navigation
Timing + Resource Timing APIs (not injected into Jaeger's app code):

| Run | Trace-API response (fetchStart → full 80k-span JSON received) | Payload | Visual state at first screenshot after navigate |
|---|---|---|---|
| 1 | 711ms | 39.7 MB | Fully rendered waterfall, all bars drawn, tree at depth 20 |
| 2 | 604ms | 39.7 MB | Fully rendered ("80000" confirmed in page text) |

Both screenshots, taken with only normal tool round-trip latency after navigation (no deliberate
wait), already showed the complete waterfall — not a skeleton, not partial rows.

**Caveat, stated plainly:** this is not the full Method Guard #2 rigor (10 runs, DevTools median +
p90) — that applies to the main T2 body's 10k/40k/100k comparative measurements, not Step 0's
early-exit check, which spec 00 scopes as "try to reproduce the claim before building any harness."
Filter/keystroke latency was not measured here — Step 0 is about the open-a-trace claim only.

## Verdict on Step 0

**Reproduced.** Jaeger v2.20.0 opens and renders an 80,000-span trace smoothly, consistent with (or
better than) its own claimed virtual-viewport capability. Data arrives and the tree is fully
rendered in well under 2 seconds — on this evidence, likely under the 1.2s "good band" threshold,
though that wasn't pinned to the decimal without the full harness.

Per spec 00: **this triggers the early exit.** The performance thesis (incumbents are slow enough
that a well-built alternative is visibly better) is falsified by Jaeger's own current behavior. The
rest of T2's harness (otel-desktop-viewer, 10k/40k/100k comparative runs, full DevTools rigor) is
not needed to reach a verdict — proceeding with it would only refine a number that already points
one way.

**Recommended read of the pre-registered table: THESIS DEAD** (or, if the owner wants the sharper
number before fully closing it out, INCONCLUSIVE-leaning-dead — never THESIS HOLDS). Per spec 00,
this does not change the build: same store, same waterfall, same linter. What it changes: no
perf-headline post; performance becomes a supporting claim, not the launch narrative; the
canvas-renderer / packed-binary tuning hours are not funded.

Spec 00 still requires *a public benchmark repo either way* — that deliverable is not yet done
(this file is the internal record, not the public write-up).
