# Spec 05 — UI surfaces (M2)

> # ⛔ BLOCKED — do not start
>
> Two things must resolve first, and neither is an executor decision.
>
> **STOP #7 — the UI framework is undecided.** `docs/prd.md` §Open Questions → UI framework row records the options as "none (Vite
> + TS)" vs "React for velocity", with the assumption being **none**: "40k virtualized rows is where
> a reconciler becomes the adversary." The call is made in **week 3, after M1, empirically.**
>
> **The M1 render strategy may pivot** (spec 02). If the browser measurement fails, the waterfall
> becomes a single `<canvas>` with hit-testing on click — which changes what a "component" even
> means here.
>
> **Note the contradiction in the source material, and do not resolve it silently.** Tasks D3–D12
> name `.tsx` paths (`ui/src/App.tsx`, `ui/src/Waterfall.tsx`, `ui/src/LintTab.tsx`), which
> presupposes React. Those paths are **illustrative, written before the framework question was
> settled** — they are not a decision. Either reading is defensible; picking one is the owner's call.
> The **`.tsx` extension and the location directly under `ui/src/` are both illustrative.** Put the
> components in one directory spec 04 does not own — `ui/src/components/` recommended. That is
> executor discretion, not a STOP item.
>
> **What to do instead:** build spec 04. Its four modules are framework-agnostic by construction and
> are the majority of the non-visual work. Everything below waits.

**Tasks:** D3 (render half), D4, D5, D6, D7
**Owns:** one components directory, `ui/src/components/` recommended. Exact filenames depend on
STOP #7. **Spec 04 owns
`ui/src/datasource.ts`, `tokens.css`, `keys.ts` and `nav.ts`** — this spec consumes them and never
edits them.
**Preconditions:** specs 01 and 04 complete; M1 cleared; STOP #7 resolved.
**Weeks:** 3–5 (M2).

---

## What M2 must deliver

Virtualized waterfall, collapse/expand, span detail with attributes fetched **on click**, attribute
search and filter — **all holding the M1-validated budgets.** If a component pushes a number past
its M1 budget, the component is wrong, not the budget.

Everything below consumes `DataSource` from spec 04 and the tokens from spec 04. **No component
calls `fetch`.**

---

## D3 (render half) — three peer tabs

**Effort:** part of D3's ~3h human / ~30min CC.

Renders the nav state from `ui/src/nav.ts`. Three peer tabs: `Traces`, `Lint`, `Receiver`. Trace
list lands; a trace opens as a layer over it with an explicit back.

The Lint tab is **separate from the Receiver panel** because they answer different questions:
the Receiver answers *"is data arriving"*, the linter answers *"is the data any good."* Do not merge
them, and do not nest one inside the other.

The **port line is a persistent orientation device** — `:4317 grpc · :4318 http` — appearing in the
tab bar, the empty state, and the Receiver panel. Reused from the approved wireframes rather than
invented.

---

## D4 — first-run empty state

**Effort:** ~3h human / ~30min CC.

This is **Story 1's first impression** and the most-seen screen in the product's life. The failure
it prevents: shipping "No traces found."

Contents: **teach the next action.**

- `Listening. No spans yet.`
- **The two env vars to copy** (the OTLP endpoint variables an SDK needs)
- **A live listener line** naming both ports with counts at zero

The alternative — stating the bare fact that there are no traces — was considered and **rejected**,
because it does not help the most common first-run failure, which is a misconfigured exporter.

> **STOP #2 — the UI port.** This screen prints the endpoint a developer copies. The UI port is
> named in no document. Ask before writing the string.

**Per-language SDK snippets are out of scope.** Two env vars cover the common case; four SDK
snippets are four things to keep correct in a tool whose whole claim is that it needs no
configuration.

---

## D5 — trace-open skeleton, no spinner

**Effort:** ~3h human / ~30min CC.

M1 permits up to **3s** to first paint on a 40k-span trace. Nothing was specified to fill that time.

Draw the frame **immediately from the trace-list summary** — span count, duration, service list, all
of which `GET /api/traces` already carries (spec 01 §6). Rows fill in afterwards.

**No spinner at any point.** A spinner says "wait"; a skeleton drawn from data you already have says
"here is your trace, the rows are arriving" — and it is honest, because the counts are real.

Frame content, immediate: service list (`checkout, db, payment`), span count (`412`), duration
(`1.23s`) rendered in the header row where the waterfall rows will appear. Rows then fill top-down
as they arrive; a row already drawn never moves once placed. If a second trace is opened before the
first finishes loading, abort the in-flight request (`AbortController`), clear the drawn rows, and
draw the new trace's skeleton — never merge rows from two traces.

---

## D6 — live trace marker, pull to apply

**Effort:** ~4h human / ~40min CC.

SSE invalidation was specified (spec 01 §6); its **appearance** never was.

- Mark a trace that is still receiving spans as live: `last span 2s ago`
- New spans arrive as an affordance: **`+14 spans — show`**
- **Nothing moves under the reader.** The update applies only on click.

A waterfall that reflows while someone is reading a span is worse than one that is briefly stale.
This is also why the SSE stream carries invalidation only and never span data — one rendering path,
and the reader decides when it runs.

---

## D7 — the Lint tab

**Effort:** ~4h human / ~40min CC.

**Approved direction: variant B** —
`~/.gstack/projects/spanfall/designs/lint-tab-20260913/wireframes.html`, block B. Choice recorded in
`approved.json` beside it.

Severity as a **3px left rule** with **group headings that name what the tier means**, rather than an
`ERROR`/`WARN`/`INFO` token the reader must already understand. The grouping carries the meaning, so
it survives greyscale, printing and colour-blindness.

> **Flat rows with a 3px left rule. Never rounded cards.** A coloured left border on a card is a
> catalogued slop pattern; it is used here *deliberately* on flat rows, and this note exists so
> nobody "improves" it into cards later.

The exact output for both scopes, the group heading strings, the ≤6-row rule, the ≤3-sample
disclosure and the empty-group rule are **specified in spec 03**, which owns them because the
terminal formatter and this tab must render the same `lint::Finding`. Do not re-derive them here.

Verify: **default scope** renders 2 groups plus the skipped line naming both cross-trace rules;
**`--all`** renders 3 groups and `all 6 rules evaluated`. A group with no firing rules is absent,
never an empty heading. ≤6 rows per run; greyscale screenshot still readable.

---

## Interaction states — every surface, every state

Build from this table; a surface missing a state is incomplete.

| Surface | Loading | Empty | Error | Success | Partial |
|---|---|---|---|---|---|
| Trace list | skeleton rows from the summary | "Listening. No spans yet." + the two env vars + live listener line | SSE dropped: "reconnecting" on the listener line | list renders | — |
| Trace view | frame + counts instantly, rows fill (**no spinner**) | n/a (reached from a list row) | `TraceEvicted`: "evicted (retention cap)" with a back link | waterfall + span detail | **live marker**, `+N spans — show`, nothing auto-moves |
| Span detail | attributes fetched on click, row stays highlighted | span with no attributes: "no attributes" | fetch fails: inline retry, **panel stays open** | attributes render | — |
| Lint tab | runs on request, button shows it working | **"no data yet"**, never "0 issues found" | a rule panics: "1 rule failed", others still shown | "clean — no findings" | skipped-rule line naming the rules and the command |
| Receiver | live counters | "no arrivals yet" on both ports | `EmptyPayload`, decode rejects, `duplicate_span` counts | **positive signal first**: "412 spans from checkout over HTTP/protobuf, 3s ago" | partial batch: accepted kept, flags shown |

**Retry/reconnect specifics** (named so the implementer isn't guessing):
- **Span detail fetch fails:** one manual "Retry" button in the panel, no auto-retry. Each click re-fetches; no attempt cap. Panel shows the error text in place of the attribute list until retry succeeds.
- **SSE dropped:** auto-retry with backoff 1s, 2s, 5s, 10s, then hold at 10s (no giving up — this is a local dev tool, the server is expected to come back). "reconnecting" on the listener line becomes "reconnecting (Nth attempt)" after the first retry. Trace viewing is unaffected; only new-span notifications pause.

---

## Acceptance

Cite the test plan: the trace-open skeleton case, the live-trace `+N spans — show` case, the
keyboard cases, and the back-from-lint-sample case.

| # | Check |
|---|---|
| 1 | Start the binary, open the browser **before sending anything**: the empty state teaches the next action. |
| 2 | Open a 40k trace with the network throttled: frame and counts appear immediately, no spinner ever. |
| 3 | Send more spans for an open trace: nothing moves until the marker is clicked. |
| 4 | Lint tab at both scopes matches spec 03's output rules exactly. |
| 5 | Collapse/expand on 40k spans: no jank, **no trace re-fetch** (assert on network, not feel). |
| 6 | Click a span: attributes arrive on demand, and the initial payload never carried them. |
| 7 | Every cell of the interaction-states table is reachable and correct. |
| 8 | M1's budgets still hold with the real components in place, not just the spike. |

## Out of scope

A responsive live UI — **desktop-only, stated.** It binds to localhost; nobody browses it from a
phone, and the export carries the narrow layout instead (spec 06). A filter box in exports. Any
animation beyond the new-span row highlight. The Receiver **panel** is M3, budgeted at 1–2 days as a
*view* over data spec 01 already produces — and it is the first thing the slip rule cuts.

## STOP items live here

**#7** blocks this entire spec. **#2** blocks D4's copy.
