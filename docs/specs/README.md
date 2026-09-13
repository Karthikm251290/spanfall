# tracescope v1 — implementation specs

**Audience:** an implementing agent (Sonnet) or a developer, executing without access to the
review conversations that produced these decisions.

**Why these files exist.** The decisions live in `docs/designs/tracescope-v1.md`, which is a
*chronological concatenation of four review passes*. It reverses two of its own decisions
partway through and carries superseded passages marked inline. It is the correct record and the
wrong input for an executor: reading it top-down and building as you go produces the wrong
system. These specs are that document flattened to current-state-only, with every reversal
already applied.

---

## Source-of-truth precedence

When two documents disagree, the higher row wins:

| Rank | Document | Role |
|---|---|---|
| 1 | `docs/specs/*` (these files) | Current state, flattened. What to build. |
| 2 | `docs/designs/tracescope-v1.md` | Why. Findings, rejected alternatives, reversals. |
| 3 | `docs/prd.md` | Product framing, architecture prose, milestones, risks. |
| 4 | `TODOS.md` | Explicitly deferred work with triggers. Not v1. |

If a spec contradicts the design doc, the spec is right and the design doc's passage is one of
the superseded ones. If a spec omits something the design doc requires, that is a spec bug —
report it, do not silently follow the design doc.

**Do not read `karthik-no-branch-eng-review-test-plan-20260913-110111.md`.** It is banner-marked
SUPERSEDED and carries three wrong architecture claims.

**Citations are by section name, never line number.** Spec 07's T14 rewrites `docs/prd.md`'s
milestone table, which reshifts every line after it. If you add a citation, cite the heading.

---

## STOP list — do not invent these

Eight things are genuinely undecided. They are not oversights and they are not yours to resolve.
**When execution reaches one, stop and ask.** Guessing any of them produces work that must be
redone, because each one propagates into many files.

| # | Undecided | Referenced by | What breaks if you guess |
|---|---|---|---|
| 1 | **The product name.** `tracescope` is a placeholder. | crate name, binary name, env-var prefix, repo URL, benchmark repo URL, every doc | T0. The spike ships a *public* repo, so the name is day-0 blocking. A guessed name gets baked into a published URL. |
| 2 | **The UI port.** `docs/prd.md` §How It Works says the web UI is served on "a third port" and never names it. | empty state, terminal output, `--open`, README, all screenshots | A guessed port appears in the first-run instructions users copy. 4317/4318 are fixed by OTLP; the third is free and unchosen. |
| 3 | **License.** Apache-2.0 is the assumption (patent grant, matches the OTel ecosystem), not a decision. `docs/prd.md` §Open Questions → license row. | `Cargo.toml`, `LICENSE`, README | Relicensing after a public commit needs every contributor's consent. |
| 4 | **Dedupe semantics on duplicate `(trace_id, span_id)`:** last-write-wins vs drop-on-duplicate. | T4, `src/store/` | OTLP permits a legitimate re-export of the same span id with *updated* fields. The two choices differ in observable output. Open sub-question recorded in T4, never resolved. |
| 5 | **`--max-memory` default:** 512 MiB / 1 GiB / 2 GiB. Assumption is 1 GiB. | retention, eviction, `docs/prd.md` §Open Questions → `--max-memory` row | Pins after M1 measures actual bytes/span. Bytes/span varies ~5x with attribute density, which is why the cap is in bytes and not spans. |
| 6 | **How many lint rules survive.** Six are specified; the outside voice argued "six rules is probably three" (rule 2 may duplicate the Receiver panel, rule 6 is a rotting table that false-positives on pinned older SDKs). | T1 → spec 03 | T1 can delete rules and move thresholds. Spec 03 is written so a cut rule is a file deletion plus one registry line, never a refactor. Do not build the engine before T1 runs. |
| 7 | **UI framework:** none (Vite + TS) vs React. Assumption is none — "40k virtualized rows is where a reconciler becomes the adversary" (`docs/prd.md` §Open Questions → UI framework row). | spec 05 entirely | Decided week 3, *after* M1, empirically. Note the contradiction this creates: D3–D12 name `.tsx` paths, which presupposes React. Those paths are illustrative, not decided. |
| 8 | **Job-search timeline.** `[PLACEHOLDER]` in the PRD's Success Criteria. | the whole plan's shape | The stated priority order (credibility → usage → learning) rests on it. The CEO review's position: if interviews are live now, the 3–4 week diagnostics wedge likely beats the 9–10 week build *regardless of the spike result*, because a credibility asset landing after the offer is worth much less. |

### Executor discretion — decide these yourself, record what you chose

Distinct from the list above. These are reversible, local, and not load-bearing: Rust edition and
toolchain version, module file layout within a crate, test-helper shape, error-crate choice
(`thiserror` vs hand-written `impl Error`), CI runner images, and the exact wording of messages
where a spec gives intent rather than a literal string. Record the choice in a comment or the
CHANGELOG; do not open a question for these.

**The name specifically:** since T0 is unresolved, do not hardcode the binary name in more than
one place. Derive it (`env!("CARGO_BIN_NAME")`, or clap's automatic name) so resolving T0 is a
one-line `Cargo.toml` change rather than a repo-wide find-and-replace.

---

## Execution order

```
  spec 00  day 0, blocking, no product code
  ├── T0 name ──────────────────────────────┐  (unblocks the public repo)
  ├── T1 rule validation ───────────────┐   │  (can cut rules → gates spec 03)
  └── T3 producer ──▶ T2 spike ──▶ GO/NO-GO │
                                        │   │
  spec 01  M0 — store + ingest ◀────────┴───┘  the foundation everything reads
  │        (span schema is the blocking dependency: docs/prd.md §Feature Dependencies)
  ▼
  spec 02  M1 — the gate. Pins --max-memory, informs the UI framework call.
  │
  ├──────────────┬────────────────┬──────────────────┐
  ▼              ▼                ▼                  ▼
  spec 03        spec 04          spec 06            spec 07
  lint           ui foundation    export             cli + demo + docs
  (needs T1)     (unblocked)      (needs 04)         (independent)
                 │
                 ▼
                 spec 05  ui surfaces — BLOCKED on STOP #7 and M1's render pivot
```

Specs 03, 04, 06 and 07 are independent once 01 lands, and their file ownership is disjoint (see
below), so they can run as parallel worktrees. This mirrors the eng review's lanes A/B/C/D, which
were checked for conflicts.

---

## File ownership — one spec owns each path

Two specs must never write the same file. If you need to touch a path another spec owns, stop and
say so rather than editing across the boundary.

| Path | Owner |
|---|---|
| `scripts/producer.*`, `scripts/spike-harness/` | 00 |
| `src/ingest/`, `src/store/trace.rs`, `src/store/writer.rs`, `src/store/mod.rs`, `src/receiver.rs`, `src/api/` | 01 |
| `bench/generator.*`, `spike/` | 02 |
| `src/lint/`, `src/store/lint_view.rs` | 03 |
| `ui/src/datasource.ts`, `ui/src/tokens.css`, `ui/src/keys.ts`, `ui/src/nav.ts`, `src/assets/fonts/` | 04 |
| `ui/src/components/` only — App, Waterfall, LintTab, EmptyState, Provenance. **Not** the four `ui/src/*.ts` modules or `tokens.css`, which are 04's. | 05 |
| `src/export/`, `ui/src/export.css` | 06 |
| `src/main.rs`, `src/cli.rs`, `src/demo.rs`, `examples/`, `docs/`, `CHANGELOG.md` | 07 |

`src/main.rs` is owned by 07 alone. Specs that need a CLI subcommand registered describe the
subcommand; 07 wires it. This is the one file every lane would otherwise touch.

---

## Task ID map

Two independent T-series exist in the design doc and collide. **Use the eng series (T0–T16) and
the design series (D1–D14).** The doc itself calls T0–T16 "the **final** ones".

| Reference | Status |
|---|---|
| eng **T0–T16** | Authoritative. `~/.gstack/projects/spanfall/tasks-eng-review-20260913-123124.jsonl` |
| design **D1–D14** | Authoritative, additive. `~/.gstack/projects/spanfall/tasks-design-review-20260913-143142.jsonl` |
| CEO **T16** (tag `--demo` traces, exclude from lint) | **Dissolved.** Demo ships as the committed fixture `examples/checkout.otlp`, not as a live span source, so there is no shared lint state to exclude it from. |
| CEO **T17** (port-conflict detection) | **Duplicate of eng T16.** Same work, two numbers. Build it once, as T16. |
| CEO **T18** (semver policy + `CHANGELOG.md`) | **ORPHAN — never carried into the eng list.** Confirmed absent from the eng JSONL. Rollback posture is "pin an older release", which makes semver and a changelog load-bearing rather than hygiene. Given a home in spec 07; needs an explicit keep-or-cut decision. |

`TODOS.md` carries a task to renumber the CEO half to C1–C3 before T0, since `docs/prd.md` now
cites T14 and there are two different T14s.

---

## How to verify

The QA input of record is
`~/.gstack/projects/spanfall/karthik-no-branch-eng-review-test-plan-20260913-122923.md` (143
lines). Each spec's Acceptance section **cites cases from it by name and adds only what is
spec-specific**. Do not restate it — two test lists drift, and this project has already hit that
failure twice.

The single most important test in the whole plan, from that file:

> **Fix-and-verify loop, no restart:** run `lint` (findings), fix the instrumentation, re-run the
> request, run `lint` again → **clean**. This must pass WITHOUT restarting tracescope; the old bad
> traces are still resident and must not affect the default run. This is the single most important
> test in this file — it is the loop the feature exists for.

## Definition of done, per spec

1. Every deliverable file in the spec exists and compiles.
2. Every row of the spec's Acceptance table passes, by running it, not by reading the code.
3. Nothing in the spec's Out-of-scope list was built.
4. Every STOP item the spec touched was asked about, not guessed.
5. Deviations are written down — in the CHANGELOG, or as a note appended to the spec.


---

## CEO Review (autoplan, 2026-09-13)

**Mode:** SELECTIVE EXPANSION (per `/autoplan` override). **Scope:** all 8 files in `docs/specs/`,
read against `docs/prd.md` and `docs/designs/tracescope-v1.md` for context.

**Voices:** Codex unavailable (`command -v codex` → not found on this machine) — tagged
`[codex-unavailable]`. The Claude second-voice subagent this phase normally dispatches (Agent
tool, fresh context, "CLAUDE SUBAGENT (CEO — strategic independence)") was **not dispatched** —
this review ran inside a forked worker session whose operating rules hard-forbid further Agent
tool calls. **Deviation, logged:** single-reviewer mode, tagged `[single-model]`. Consensus table
below is entirely N/A, not a disagreement.

This is the 5th review pass on this repo today (after 3 CEO, 3 eng, 1 design, 1 outside-voice
pass — see the superseded "stop reviewing, start building" decision). The brief for this pass was
explicit: adjudicate the 8-item STOP list and CEO T18, and find genuinely new gaps in the
*flattened* specs — not re-derive settled architecture. Findings below are calibrated to that.

### Step 0A — Premise Challenge

1. **Is this the right problem to solve?** Yes, and this was already validated three times today
   with primary-source evidence (OTel DevEx survey, Jaeger/Grafana issue numbers). Not re-litigated.
2. **Right framing, or a proxy problem?** This is where new ground exists. The PRD's own priority
   order is *credibility (job search) > usage > learning*, and the stated wedge decision criterion
   — whether a Rust viewer beats incumbents on 10k/40k spans — is a **feasibility** question. The
   **business** question, "does this get cited in a job conversation in time to matter," is gated
   on `[PLACEHOLDER — tied to job-search timeline]` in the Success Criteria (STOP #8). A 9–10 week
   build whose entire performance thesis could resolve THESIS DEAD in week 2 (spec 00's T2, spec
   02's M1) is being scoped as if the timeline question doesn't exist. It does, and it's the one
   the CEO review already flagged and never got an answer to. **This is the top User Challenge
   below.**
3. **What would happen if we did nothing?** Real pain, well-evidenced (Jaeger #562, #645, Grafana
   #114264). Not new.

### Step 0B — Existing Code Leverage

No existing codebase — greenfield, solo, pre-first-commit. "What already exists" is therefore
about *upstream* leverage, all already named and correctly not rebuilt: `opentelemetry-proto` for
wire types, `tonic`/`axum` for transports, OTel Collector's file exporter for T1's validation (no
tracescope code needed), the OTel demo app as a real-telemetry control for both T2 and M1. Nothing
in the flattened specs quietly reinvents any of these.

### Step 0C — Dream State Mapping

```
CURRENT STATE                    THIS PLAN (v1, 9-10wk)              12-MONTH IDEAL
No repo, no code,        --->    Ingest+store+lint+export+CLI, --->  Peer-of-incumbents local
3 CEO/3 eng/1 design              web UI, single binary,               viewer: persistence,
reviews done today                credibility benchmark shipped       diffing, terminal-first
                                                                       output, MCP query surface
                                                                       for agents (TODOS.md)
```
This plan moves toward the ideal — every v1.1/Stage-2/Stage-3 item in `TODOS.md` builds directly
on v1's store and query layer. No architectural choice in the flattened specs forecloses any of
those (persistence is additive protobuf framing; diffing needs nothing v1 doesn't already have;
the MCP server is "cheap to build on top of the existing query layer"). No dream-state regression
found.

### Step 0C-bis — Implementation Alternatives

Not re-litigated: the architecture (trace-partitioned struct-of-arrays, no query engine) was
decided 2026-09-13 via `/plan-eng-review` and stress-tested (Opus pass, two adversarial rounds,
outside voice). Re-opening it without a cross-trace-search requirement would violate the
project's own settled-decision discipline (`TODOS.md` watch item). Auto-decided: **hold**, per
DRY + boil-the-ocean (re-deriving a settled architecture is waste, not rigor).

One alternative genuinely still open and correctly gated, not decided here: **APPROACH A (ship
v1 as specced, 9-10wk)** vs **APPROACH B (ship the 3-4 week diagnostics wedge — Story 1 + linter +
Receiver + demo, skip the performance moonshot and defer M1/waterfall)**. This is STOP #8's real
shape as an implementation-alternatives question, not just a metrics placeholder. Queued as the
User Challenge below rather than auto-decided, because it changes what "v1" ships.

### Step 0D — Mode-Specific Analysis (SELECTIVE EXPANSION)

Complexity check: the plan touches far more than 8 files (it's a whole product), but each spec's
file-ownership table is disjoint and the specs are explicitly designed to run as parallel
worktrees — this is scope-appropriate for a full product, not scope creep. No complexity smell.

Cherry-pick scan (candidates only, not yet added to scope):
- **CEO T18 (semver + CHANGELOG)** — orphaned, spec 07 itself recommends keep. Effort ~1h/~15min
  CC, in blast radius (spec 07 already owns `CHANGELOG.md`), non-duplicate. **Auto-approved**
  (mechanical: <1 day CC, in blast radius, recommended by the spec itself). Not a taste call.
- **Eviction-thrashing guard** (new, Section 7 below) — S effort. **Marked TASTE DECISION** below;
  borderline whether it's in v1 scope or a `TODOS.md` item.
- No other expansion candidates found worth surfacing — the specs are already unusually
  boil-the-ocean (D1-D14, six lint rules, full accessibility pass, XSS hardening on 10 hostile
  fields) for a solo v1. Cherry-picking further here would be scope creep against the plan's own
  stated priorities, not rigor.

### Step 0E — Temporal Interrogation

```
HOUR 1 (T0/T1/T2, day 0):   Name + license unblock the repo. T1's premise (2-3 real instrumented
                             projects exist) must be checked before "half a day" is trusted.
HOUR 2-3 (M0, week 1):      Span-schema shape is now genuinely frozen (spec 01 says so explicitly)
                             — the STOP #4 dedupe semantics (last-write-wins vs drop) should be
                             resolved before or during M0, not deferred, since T4 rides the same
                             insert-time probe as parent resolution.
HOUR 4-5 (M1, week 2):      STOP #7 (framework) and STOP #5 (--max-memory default) both resolve
                             here. Spec 05 is fully blocked until then — correctly sequenced.
HOUR 6+ (M2-M3, weeks 3-6): STOP #8's timeline question should have been answered by week 1, not
                             week 6 — if the answer is "interviews are live now," discovering that
                             in week 6 wastes 5 weeks that the diagnostics wedge alone would not
                             have.
```

### Step 0F — Mode Confirmation

SELECTIVE EXPANSION, per `/autoplan` override. Confirmed; not drifted.

---

### Step 0.5 — Dual Voices

```
CEO DUAL VOICES — CONSENSUS TABLE:
═══════════════════════════════════════════════════════════════
  Dimension                           Claude  Codex  Consensus
  ──────────────────────────────────── ─────── ─────── ─────────
  1. Premises valid?                   Yes*    N/A    N/A
  2. Right problem to solve?           Yes     N/A    N/A
  3. Scope calibration correct?        Mostly† N/A    N/A
  4. Alternatives sufficiently explored?Yes    N/A    N/A
  5. Competitive/market risks covered? Yes     N/A    N/A
  6. 6-month trajectory sound?         Yes     N/A    N/A
═══════════════════════════════════════════════════════════════
*Valid except STOP #8's timeline gap, surfaced as User Challenge.
†Correct for the build; the timeline-vs-wedge tradeoff (STOP #8) is unresolved, not miscalibrated.
N/A = no second voice available this run (see Voices note above) — not a disagreement.
```

---

### Sections 1–10 (full review)

**Section 1 — Architecture.** Not re-evaluated from scratch: this architecture was decided,
stress-tested, and is now cited by name in every spec as settled (`docs/prd.md` §Architecture,
§Decided Against). No new architectural finding. One observation worth naming: the file-ownership
table (`docs/specs/README.md`) is itself the dependency graph — it's unusually explicit for a
solo project, which is a good sign, not a gap.

**Section 2 — Error & Rescue Map.** Reviewed against spec 01's reject taxonomy (`EmptyPayload`,
`DecodeError`, `duplicate_span`, `late_span_after_eviction`), spec 03's per-rule `catch_unwind`,
and spec 06's export error table. All rescued paths name a user-visible message; none swallow
silently. **No new gap** — this is one of the most thorough error/rescue mappings I've seen in a
pre-code plan.

**Section 3 — Security & Threat Model.** Spec 06 (export) already treats this as the highest-stakes
spec in the project and covers the real threat model correctly: stored XSS via `<` JSON
escape + `textContent`-only rendering (not HTML-escaping, correctly distinguished), CSP as defense
in depth, and a committed secret-scan pattern list with stderr/stdout stream split. **No new
finding.** One thing to flag for completeness, not as a gap: the secret-scan pattern list (T13) is
static and known-incomplete by design (only 7 key patterns, 3 value shapes) — this is explicitly
accepted residual risk in the spec, correctly not silently declared "safe."

**Section 4 — Data Flow & Interaction Edge Cases.** The interaction-states table in spec 05
(Loading/Empty/Error/Success/Partial × 5 surfaces) is exactly the shape this section asks for, and
it's already complete. No cell is missing. **No new finding.**

**Section 5 — Code Quality.** Spec 01's `deny`-lint on `unwrap`/`expect`/`panic`/indexing in the
ingest module, and the registry-array pattern for lint rules (cutting one rule is "a file deletion
plus one line") both directly address DRY and over-engineering concerns before they exist. One
item already self-flagged by spec 03 and correctly not yet resolved: **rule 1's cardinality ratio
threshold is mathematically close to unreachable at `--all` scope** as trace count grows (distinct
names grow sub-linearly, total spans grow linearly, so `distinct/total` falls over time) — spec 03
already assigns this to T1 to measure rather than deciding it here. Correctly deferred, not a gap.

**Section 6 — Test Review.** Every spec cites the shared test-plan file
(`karthik-no-branch-eng-review-test-plan-20260913-122923.md`) by name rather than restating it, per
`docs/specs/README.md`'s own anti-drift rule. Verified that file exists on disk. Each spec's
acceptance table adds only spec-specific cases on top. **No new gap.**

**Section 7 — Performance.** One genuinely new finding: **no explicit guard against eviction
thrashing.** If sustained ingest rate exceeds what `--max-memory` can hold even for freshly-arrived
traces, the store could evict-and-recreate traces in a tight loop (each evict frees memory a new
trace immediately consumes), burning CPU on eviction bookkeeping without ever holding a usable
working set. Spec 01 §5 specifies *which* trace gets evicted (oldest last-activity) but not a
floor/backpressure interaction for this case. **Marked TASTE DECISION** (below) — plausibly out of
v1 scope (needs load beyond what T2/M1's benchmarks generate), plausibly a one-line note in
`TODOS.md` as a watch item alongside the existing cross-trace-search tripwire.

**Section 8 — Observability & Debuggability.** The Receiver panel (spec 01 §7, spec 05's
interaction table) is designed as first-class observability for the tool's own ingest, and `--dump`
plus `/api/receiver` are explicitly the fallback if M3 gets cut by the slip rule. This *is* the
observability section, essentially — a tool whose entire value prop is observability has better
self-observability discipline than most reviewed plans get for their actual features. **No new
gap.**

**Section 9 — Deployment & Rollout.** Single static binary, no migration, no feature flags needed
for v1 (nothing is behind one). CEO T18 (semver + CHANGELOG) is exactly this section's concern —
"rollback is the user pins an older tag" makes changelog quality part of the rollback plan, which
is why it's auto-approved above rather than deferred. **No other new gap.**

**Section 10 — Long-Term Trajectory.** Reversibility: the architecture is a **1-way door already
walked through and validated** (struct-of-arrays, no query engine) — rated 2/5 reversible by
design (deliberately, per the `TODOS.md` watch item: reversing it needs a real cross-trace-search
requirement, not a preference). Path dependency is healthy: v1.1 (persistence, diffing, logs) and
Stage 3 (cloud pull) both build additively on the v1 query layer without needing to unwind
anything. **No new finding** beyond what's in `TODOS.md` already.

**Section 11 — Design & UX.** Already run today by `/plan-design-review` (D1-D14, this pass just
consumes its output). Not re-run at CEO depth — that would duplicate a same-day review rather than
add signal. One cross-cutting note: `TODOS.md`'s deferred "terminal-first tree output" item names a
real tension with the CEO review's own stated "platonic ideal is terminal-first" — v1 ships
web-first instead. This is already recorded as a conscious, deferred tension in `TODOS.md`, not a
silent contradiction. **No new finding**, but worth naming here since Section 11 is where a CEO
pass would normally catch it.

---

### Required Outputs

**"NOT in scope"** (already correctly deferred per spec, no changes recommended):
cross-trace search/index (watch item, `TODOS.md`), CI assert mode, `--json` lint output, lint
threshold config, subtree export, `--host`/non-localhost bind, per-language SDK snippets in the
empty state, vim keybindings, responsive live UI, TUI mode, watch-mode diffing, MCP query server.
All correctly named with triggers in `TODOS.md` or the owning spec's Out-of-scope section.

**"What already exists"**: see Step 0B above — no rebuild-vs-reuse issue found.

**Error & Rescue Registry**: see Section 2. Complete; sourced from spec 01 §1-2 and spec 06 T10-T13.
No CRITICAL GAP (RESCUED=N + TEST=N + SILENT) found in any of the eight specs.

**Failure Modes Registry**:
```
  CODEPATH                    | FAILURE MODE            | RESCUED? | TEST? | USER SEES?        | LOGGED?
  ----------------------------|-------------------------|----------|-------|-------------------|--------
  ingest::convert (envelope)  | truncated protobuf       | Y        | Y     | typed reject      | Y
  ingest::convert (per-span)  | malformed one span       | Y (flag) | Y     | flagged, kept     | Y
  writer send()                | ingest-timeout exceeded  | Y        | Y     | 429/RESOURCE_EXH. | Y
  store insert                 | duplicate (trace,span)   | Y        | Y     | counted, dropped  | Y
  store insert                 | orphan parent            | Y (flag) | Y     | rendered as root  | Y
  eviction                     | trace re-arrives after   | Y (flag) | Y     | RESURRECTED flag  | Y
                                | eviction ("resurrected") |          |       |                   |
  eviction (NEW, this review)  | sustained ingest > cap,  | N ← WATCH| N     | none (silent CPU  | N
                                | thrash evict/recreate    |          |       | burn, no counter) |
  export write                 | disk full mid-write      | Y        | Y     | OS error verbatim | N/A (no partial)
  lint rule eval                | rule panics              | Y        | Y     | "1 rule failed"   | Y
  global interner               | key/service-name cap hit | Y        | Y     | Receiver warning  | Y
```
One row is new and not yet a WARN in any spec (eviction thrashing) — recommend a one-line
`TODOS.md` watch item, not a v1 blocker (see Taste Decisions).

**Dream state delta**: v1 ships the full local-viewer wedge with headroom into v1.1/Stage 2/3
already built into the storage and query design — see Step 0C. No delta correction needed.

**Completion Summary**:
```
+====================================================================+
|            MEGA PLAN REVIEW — COMPLETION SUMMARY (Phase 1: CEO)    |
+====================================================================+
| Mode selected        | SELECTIVE EXPANSION                          |
| System Audit         | 8 spec files + prd.md + designs doc read     |
| Step 0               | STOP #8 reframed as User Challenge           |
| Section 1  (Arch)    | 0 new issues — architecture settled today    |
| Section 2  (Errors)  | 0 GAPS — reject taxonomy complete            |
| Section 3  (Security)| 0 new issues — export threat model complete  |
| Section 4  (Data/UX) | 0 unhandled — interaction table complete     |
| Section 5  (Quality) | 0 new (rule-1 threshold already self-flagged)|
| Section 6  (Tests)   | 0 gaps — shared test plan cited correctly    |
| Section 7  (Perf)    | 1 new: eviction-thrashing watch item         |
| Section 8  (Observ)  | 0 gaps — Receiver panel is the observability |
| Section 9  (Deploy)  | 0 risks beyond CEO T18 (resolved: keep)      |
| Section 10 (Future)  | Reversibility: 2/5 (deliberate), 0 new debt  |
| Section 11 (Design)  | 0 new — covered by same-day design review    |
+--------------------------------------------------------------------+
| NOT in scope         | written (12 items, all pre-existing)         |
| What already exists  | written                                      |
| Dream state delta    | written — no correction needed               |
| Error/rescue registry| 10 rows, 0 CRITICAL GAPS, 1 WATCH item added |
| Failure modes        | 10 total, 0 CRITICAL, 1 WATCH (eviction)     |
| TODOS.md updates     | 1 proposed (eviction thrashing watch item)   |
| Scope proposals      | 2 proposed (T18 keep, eviction guard), 1 auto|
|                       | -approved (T18), 1 taste-decision (eviction) |
| CEO plan             | written: ceo-plans/2026-09-13-autoplan-      |
|                       | phase1.md                                    |
| Outside voice        | unavailable (codex + subagent both skipped)  |
| Diagrams produced    | 3 (dream state, temporal, failure modes)     |
| Stale diagrams found | 0                                             |
| Unresolved decisions | 1 User Challenge (STOP #8), 2 Taste Decisions|
+====================================================================+
```

### Unresolved Decisions (queued for the /autoplan Final Approval Gate — never auto-decided)

**USER CHALLENGE 1 — STOP #8, job-search timeline vs. 9-10 week build**
- **What the plan assumes:** that building the full v1 (linter + export + web UI + benchmark) over
  9-10 weeks is the right shape regardless of when the credibility asset needs to exist.
- **What this review recommends:** answer the job-search timeline question NOW, before M0 starts,
  not after M1's spike. If interviews/applications are active in the next 4-6 weeks, the 3-4 week
  diagnostics wedge (Story 1 + linter + Receiver + `--demo`, skip M1's browser spike and the
  waterfall) is a more direct path to a citable artifact than a 9-10 week build whose performance
  thesis might resolve THESIS DEAD at week 2 anyway.
- **Why:** the PRD's own priority order (credibility > usage > learning) is not reflected in the
  milestone plan's sequencing, which builds toward the full benchmark story regardless of timeline
  pressure. This exact tension is already named in `docs/specs/README.md`'s STOP list — this
  review is not inventing it, only insisting it get answered rather than carried forward a 6th
  time.
- **Cost if wrong (built the 9-10wk version, but timeline was tight):** 5+ weeks spent on a
  performance thesis that, per the M1 pivot plan, may not even ship as the headline narrative.
- **Cost if wrong (built the wedge, but timeline was not tight):** a smaller, faster-shipping v1
  that undersells the systems-engineering credibility angle — recoverable by continuing to M1-M3
  afterward, since nothing in the wedge forecloses the rest.

**TASTE DECISION 1 — CEO T18 (semver + CHANGELOG)**
- Recommendation: **keep**, per spec 07's own argument (rollback = pinning an older tag, so
  changelog quality is part of the recovery path, not hygiene). Auto-approved above as mechanical
  (≤1h, in blast radius, no duplicate). Listed here only because it was an orphaned decision the
  gate should see was actually resolved, not silently dropped a second time.

**TASTE DECISION 2 — eviction-thrashing watch item**
- Recommendation: add a one-line `TODOS.md` watch item (P3, alongside the existing cross-trace-
  search tripwire), not a v1 scope addition. Viable alternative: add a floor/backpressure guard to
  spec 01 now (S effort, ~half day). Impact if deferred and wrong: under sustained overload beyond
  what M1's benchmarks generate, CPU burns on evict/recreate cycling with no counter to see it by —
  discoverable via the Receiver's eviction counters once it happens, so it's not silent forever,
  just not designed against in advance.

**Codex/subagent tension:** none — no second voice ran this pass (see Voices note).

<!-- AUTONOMOUS DECISION LOG -->
## Decision Audit Trail

| # | Phase | Decision | Classification | Principle | Rationale | Rejected |
|---|-------|----------|-----------------|-----------|-----------|----------|
| 1 | CEO | Hold the settled architecture (trace-partitioned struct-of-arrays, no query engine) — do not re-open | Mechanical | DRY / boil-the-ocean | Already decided 2026-09-13 via /plan-eng-review, stress-tested twice; re-deriving it would waste the review, not add rigor | Re-evaluating alternatives from scratch |
| 2 | CEO | Auto-approve CEO T18 (semver policy + CHANGELOG.md) as v1 scope | Mechanical | Boil-the-ocean (<1 day CC, in blast radius) | Spec 07 already argues to keep it; rollback = pinning an older tag makes changelog part of the recovery path | Leaving it orphaned a second time |
| 3 | CEO | STOP #8 (job-search timeline vs. 9-10wk build) queued as User Challenge, not auto-decided | User Challenge | N/A — never auto-decided | Both the plan's own priority order and this review's independent read suggest the timeline should gate scope, but only the user has the actual timeline | Silently keeping the 9-10wk plan as-is |
| 4 | CEO | Eviction-thrashing guard queued as Taste Decision (recommend TODOS.md watch item over v1 scope addition) | Taste | Pragmatic (S effort either way, no clear win) | No load-generating scenario in T2/M1 benchmarks currently exercises this; Receiver's eviction counters make it discoverable, not silent, if it occurs | Adding a v1 scope item without benchmark evidence it's needed |
| 5 | CEO | Sections 1-6, 8-11: no new findings beyond prior reviews — logged as "examined, nothing new" rather than skipped | Mechanical | N/A (anti-skip rule) | Verified against spec content directly (see Sections 1-11 above) rather than assumed clean | N/A |
| 6 | Design | Add export filename format (`{service}-{trace-id-short}-{UTC-timestamp}.html`, printed to stdout) to spec 06 T10 | Mechanical | Explicit-over-clever / completeness | Spec 06 referenced a printed path (acceptance #6, T13's stream split) but never named the filename scheme — genuine gap, not a STOP item, cheap and unambiguous to fix | Leaving it to the implementer to invent silently |
| 7 | Design | Add an inline output example for "1 rule failed" placement to spec 03 | Mechanical | Completeness | States table said the string but never showed where it renders relative to the groups/footer | Leaving formatting to implementer guesswork |
| 8 | Design | Add trace-list skeleton content spec + race-condition (AbortController) handling to spec 05 D5 | Mechanical | Explicit-over-clever | M1's 3s render budget is gated on this component but the spec described intent only, not the frame's actual content or the concurrent-open case | Leaving as prose-only, unspecified race behavior |
| 9 | Design | Add retry/reconnect specifics (span-detail retry button, SSE backoff schedule) to spec 05's interaction-states table | Mechanical | Completeness | "Inline retry" and "reconnecting" were named states with no behavior attached | Leaving retry cadence and give-up behavior to implementer discretion |
| 10 | Design | Reject: independent-subagent Finding 1a (port discovery "completely broken") reframed, not treated as a new gap | Taste | N/A — correction, not a decision | Spec 07 line 39 and spec 05 line 80 already name STOP #2 as surfacing at exactly this spot (startup print). It's deliberately withheld pending the port decision, not an oversight | Auto-fixing STOP #2 (out of scope — it's the one class of item never guessed) |
| 11 | Design | Reject: independent-subagent Finding 3a (fix-and-verify loop breaks if old trace still resident) | Taste | N/A — correction, not a decision | Spec 03 acceptance test #1 explicitly tests this exact case ("clean, with the old bad traces still resident") and test #2 covers `--all` still reporting them. Already covered, not a gap | Adding a redundant acceptance test |
| 12 | Design | Reject: independent-subagent Finding 4e (severity left-rule fails greyscale) and the "wire generation counter into UI" suggestion | Taste | N/A — correction, not a decision | Spec 03/05 already state color is reinforcement, heading text carries meaning (survives greyscale by design); spec 03's "what NOT to build" list explicitly rejects generation-checked links inside lint | Reopening settled decisions without new evidence |
| 13 | Design | Reject: independent-subagent's tab-reorder suggestion (Receiver before Lint) | Taste | Pragmatic | Spec 04 D3's tab order (Traces, Lint, Receiver) is settled; reordering nav is a structural change on a hypothetical confusion, not an observed one | Reordering nav on speculation |
| 14 | Design | Focus-ring exact styling (border width, inset/outset, scroll-animation) left as executor discretion, not specified further | Taste | Pragmatic | Low risk if unspecified; `docs/specs/README.md`'s existing "Executor discretion" list already covers comparable small implementation choices | Blocking on a cosmetic detail with no clear right answer |
| 15 | Design | Lint scope-label behavior at 10k+ resident traces: not addressed, no TODOS item added | Taste | Pragmatic | Speculative edge case — v1's own eviction/`--max-memory` behavior (STOP #5) makes it unclear whether traces stay resident at that scale at all; premature to spec | Adding a TODOS watch item with no evidence it's needed |

---

## Design Review (autoplan, 2026-09-13)

**Mode:** SELECTIVE EXPANSION overrides apply — every AskUserQuestion in the loaded skill auto-decides.
**Scope:** all 8 files in `docs/specs/`, read directly (not from memory) against `docs/designs/tracescope-v1.md`'s token block and the CEO review's findings above.

**Voices:** Codex unavailable, tagged `[codex-unavailable]`. The independent Claude design subagent
(fresh context, zero prior-phase visibility, dispatched directly by the parent /autoplan session
since forked workers cannot spawn their own Agent calls) ran and reported 6 numbered findings plus
a 10-row ambiguous-decisions table. This reviewer read all 8 spec files directly and verified every
subagent finding against the actual spec text before accepting or rejecting it — 3 of 6 findings did
not hold up (see Decision Audit Trail rows 10-12 above) once checked against spec content the
subagent's prompt didn't ask it to cross-reference (STOP list, "what NOT to build" list).

### Step 0 — Design Scope Assessment

**0A. Initial rating: 7/10.** The specs are unusually design-complete for a pre-code plan — full
interaction-states table (spec 05), a settled token block (spec 04 D8), explicit anti-slop
discipline (flat rows not cards for lint, spec 03/05), accessibility specifics (spec 04 D12: ARIA
tree semantics, focus tracking by span index, 44px touch targets). What kept it from 9-10: three
small implementation-detail gaps (export filename, retry/reconnect cadence, race-condition handling)
that a careful implementer would hit and have to invent silently. All three are now fixed directly
in the specs (Decision Audit Trail rows 6, 8, 9) — this review does not re-rate after its own fixes
since /autoplan auto-decides rather than iterating with the user, but the gaps that justified <10
are closed.

**0B. DESIGN.md status:** none exists. Already a known, deferred gap — `TODOS.md`'s "A real
DESIGN.md" item (P3) and the prior settled decision ("~15 CSS variables... not a DESIGN.md") both
already cover this. Not re-flagged as new; the token block (spec 04 D8) is a real, if minimal,
design system and is what all 8 specs correctly calibrate against.

**0C. Existing design leverage:** the approved Lint tab wireframe
(`~/.gstack/projects/spanfall/designs/lint-tab-20260913/wireframes.html`, variant B, recorded in
`approved.json`) is correctly cited and reused by spec 05 D7 rather than re-derived. No UI pattern
in the flattened specs invents something a prior review already resolved.

### Step 0.5 — Dual Voices

```
DESIGN LITMUS / CONSENSUS TABLE:
═══════════════════════════════════════════════════════════════
  Dimension                              Primary  Subagent  Consensus
  ─────────────────────────────────────── ─────── ───────── ─────────
  1. Information hierarchy sound?          Yes      Partial   DISAGREE→resolved: gaps were
                                                                real but small, now fixed
  2. Missing states identified correctly?  Mostly   Yes       CONFIRMED (retry/reconnect gaps)
  3. User journey / emotional arc sound?   Yes      Partial   DISAGREE: fix-and-verify claim
                                                                did not hold (spec already covers it)
  4. Specific UI vs. generic patterns?     Yes      N/A       CONFIRMED (subagent didn't probe
                                                                AI-slop dimension directly)
  5. Design-decisions-that-haunt found?    Partial  Yes       CONFIRMED (export filename, real)
═══════════════════════════════════════════════════════════════
Codex: N/A (unavailable this session, not a disagreement).
```

Full independent subagent output is preserved verbatim in the autoplan session transcript (not
duplicated here — see the Decision Audit Trail for the specific accept/reject calls made against
each of its 6 numbered findings and 10-row table).

### Passes 1-7

**Pass 1 — Information Architecture: 8/10.** Three peer tabs (Traces/Lint/Receiver), trace list as
landing surface, trace-as-layer-with-explicit-back (spec 04 D3, state half). Constraint worship
applied correctly: D4's empty state shows exactly three things (status line, two env vars, listener
line) — not a generic empty state. Gap: none new. The subagent's tab-reorder suggestion (Receiver
before Lint) was considered and rejected (audit row 13) — the order is settled and the concern is
hypothetical, not observed.

**Pass 2 — Interaction State Coverage: 9/10 → 9.5/10 after fixes.** The interaction-states table
(spec 05) already covers Loading/Empty/Error/Success/Partial across all 5 surfaces — CEO review
already confirmed no cell is missing. This pass's contribution: two named-but-unspecified states
(span-detail retry, SSE reconnect) now have concrete behavior (audit row 9). Empty states are
treated as features throughout — D4's empty state and Lint's "no data yet" (never "0 issues found")
both correctly distinguish "nothing happened yet" from "checked, found nothing."

**Pass 3 — User Journey & Emotional Arc: 8/10.** D5 (skeleton, no spinner) and D6 (live marker,
pull-to-apply, "nothing moves under the reader") are a coherent, deliberately calm emotional arc —
the explicit design principle is that a waterfall reflowing mid-read is worse than a briefly stale
one. The subagent's claimed break in this arc (fix-and-verify loop fragility) does not hold —
verified against spec 03's actual acceptance tests, which already test the "old trace still
resident" case explicitly (audit row 11).

**Pass 4 — AI Slop Risk: 9/10.** Classifier: OPERATE (App UI) for the live viewer, mixed for the
export (still App UI — view-only, no marketing surface). Zero hard-rejection patterns found: no
card grids (lint uses flat rows with a left rule specifically to avoid the cataloged "colored
left-border on cards" slop pattern — spec 05 D7 calls this out by name, unprompted), no gradient
backgrounds, no generic hero copy (there is no hero), IBM Plex Sans/Mono named explicitly (not
`system-ui`, the "gave up on typography" tell). Universal rules already followed: CSS variables for
color (spec 04 D8), tabular numerals on durations/counts, no default font stacks. The 0.5 point held
back: the design token block doesn't yet theme browser-native surfaces (selection, caret, scrollbar)
per D8's own stated rule — flagged in D8's prose already, no new finding.

**Pass 5 — Design System Alignment: 7/10.** No DESIGN.md (known, deferred, see 0B). The token block
(spec 04 D8) is the de facto system and every spec correctly cites it rather than inventing new
values. Not re-litigated: this was a settled decision today (the prior "Design tokens" active
decision).

**Pass 6 — Responsive & Accessibility: 8/10.** Spec 04 D12 (ARIA tree semantics, `aria-rowcount` on
the virtualized total, focus-by-span-index, keyboard model with 2-stage Escape) and spec 06 D11
(export-only responsive layout, 44px touch targets, full-width detail sheet under 700px) are both
specific, not aspirational. Live UI is deliberately desktop-only (a conscious, recorded tradeoff, not
an oversight — `TODOS.md`'s "Responsive layouts for the live UI" item already names the trigger). Gap
kept out of 9-10: focus-ring exact styling (border width, inset/outset, scroll-into-view animation)
is unspecified — left as executor discretion (audit row 14), genuinely low-risk.

**Pass 7 — Unresolved Design Decisions.**
```
DECISION NEEDED                          | IF DEFERRED, WHAT HAPPENS
------------------------------------------|---------------------------
STOP #2 (UI port)                        | D4's empty state and the startup print can't be
                                          | written concretely — already correctly gated, not
                                          | new, not resolved here (owner's call).
STOP #7 (UI framework: none vs React)    | Spec 05 stays fully blocked; D3-D12's .tsx paths
                                          | stay illustrative only, per spec 05's own banner.
STOP #6 (how many of 6 lint rules survive)| Spec 03's output examples (2 vs 3 groups) may need
                                          | revision after T1 measures; not a design gap, an
                                          | input this pass correctly treats as pending.
```
All three were already correctly identified as STOP items by prior passes; this pass adds no new
unresolved design decision. Zero design-only ambiguities remain undecided after this review's fixes.

### Required Outputs

**"NOT in scope"**: light/dark toggle (system-preference only, settled), a full DESIGN.md (token
block is the v1 system), vim keybindings + `?` overlay (`TODOS.md` P3), responsive live UI
(`TODOS.md` P3), any entrance/transition motion beyond the new-span row highlight. All pre-existing,
none newly deferred by this pass.

**"What already exists"**: the approved Lint tab wireframe (variant B) and its `approved.json`; the
~15-variable token block (spec 04 D8); the ARIA/keyboard model (spec 04 D12). All correctly reused,
not reinvented, by every spec that touches UI.

**TODOS.md updates**: none. Every small gap found was cheap and unambiguous enough to fix directly
in the relevant spec (audit rows 6, 8, 9) rather than deferred — consistent with the plan's own
stated bias toward fixing over accumulating TODOs for anything under ~1h of spec-writing effort.

### Implementation Tasks
Synthesized from this review's findings. Each task derives from a specific finding above.

- [ ] **D-T1 (P2, human: ~20min / CC: ~5min)** — export — Implement the filename format now specified in spec 06 T10
  - Surfaced by: Pass 3 / Pass 7 (design-decisions-that-haunt) — export gave no file confirmation
  - Files: `src/export/`
  - Verify: `tracescope export <id>` prints a path matching `{service}-{trace-id-short}-{UTC-timestamp}.html`
- [ ] **D-T2 (P2, human: ~15min / CC: ~5min)** — lint — Render the "1 rule failed" line per the new spec 03 example
  - Surfaced by: Pass 2 (missing states) — panic recovery had no rendering example
  - Files: `src/lint/`
  - Verify: deliberately panicking rule yields the line in the position spec 03 now shows
- [ ] **D-T3 (P1, human: ~1h / CC: ~15min)** — ui/waterfall — Implement skeleton content + AbortController race handling per spec 05 D5
  - Surfaced by: Pass 3 (user journey) — M1's render budget is gated on this component
  - Files: `ui/src/components/` (exact path pending STOP #7)
  - Verify: throttled-network 40k-span open shows immediate frame+counts; opening a second trace mid-load shows no mixed rows
- [ ] **D-T4 (P2, human: ~30min / CC: ~10min)** — ui — Implement span-detail retry button and SSE backoff schedule per spec 05
  - Surfaced by: Pass 2 (missing states)
  - Files: `ui/src/components/`
  - Verify: kill SSE connection, observe "reconnecting (Nth attempt)" backoff sequence; span-detail fetch failure shows a working Retry button

### JSONL artifact
Written to `~/.gstack/projects/spanfall/tasks-design-review-<TIMESTAMP>.jsonl` (see below).

### Completion Summary
```
+====================================================================+
|         DESIGN PLAN REVIEW — COMPLETION SUMMARY (Phase 2)          |
+====================================================================+
| System Audit         | 8 spec files read directly, verified vs.    |
|                       | independent subagent's 6 findings          |
| Step 0               | 7/10 initial, gaps fixed directly           |
| Pass 1  (Info Arch)  | 8/10 — no new gap                           |
| Pass 2  (States)     | 9/10 → 9.5/10 — 2 states specified          |
| Pass 3  (Journey)    | 8/10 — 1 subagent claim rejected (verified) |
| Pass 4  (AI Slop)    | 9/10 — 0 hard rejections, anti-slop by design|
| Pass 5  (Design Sys) | 7/10 — DESIGN.md gap known & deferred       |
| Pass 6  (Responsive) | 8/10 — focus-ring styling left to executor  |
| Pass 7  (Decisions)  | 0 new unresolved; 3 pre-existing STOP items |
+--------------------------------------------------------------------+
| NOT in scope         | written (5 items, all pre-existing)         |
| What already exists  | written                                     |
| TODOS.md updates     | 0 — all gaps fixed directly instead         |
| Decisions made       | 4 direct spec fixes (audit rows 6,8,9 x2)   |
| Decisions deferred   | 0                                           |
| Consensus            | 2/5 confirmed, 2 disagreements (resolved by |
|                       | direct verification), 1 not-probed          |
| Overall design score | 7/10 → 8.2/10 (avg of passes 1-6)           |
+====================================================================+
```

### Unresolved Decisions (queued for the /autoplan Final Approval Gate)

None from this phase. All findings were either mechanically auto-fixed (small, unambiguous) or
resolved as taste-decision corrections against the independent subagent (documented in the Decision
Audit Trail, rows 10-15) — none rose to the level of a User Challenge (no finding here suggested the
user's stated scope/direction should change).
