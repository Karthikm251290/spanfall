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

Eight things were genuinely undecided. They were not oversights and were not the executor's to
resolve. **Six are now resolved** — four at the `/autoplan` Final Approval Gate (see
`## Final Approval Gate (autoplan, 2026-09-13)` below), plus #1 and #3 resolved via T0 the same day
(collision check + license, see spec 00). Two remain open (**#6**, pending T1; **#7**, pending M1):
**when execution reaches one of those, stop and ask.** Guessing either produces work that must be
redone, because each propagates into many files.

Note: `tracescope` still appears throughout these specs' prose as the placeholder name it was
written with. T0's own scope was narrow — Cargo.toml, LICENSE, and "no *source* file hardcodes it"
(now true: `src/main.rs` derives it via `env!("CARGO_BIN_NAME")`) — not a find-and-replace across
every doc. Treat every `tracescope` in prose below as `spanfall`; rewriting the prose itself is
optional polish, not a blocker.

| # | Undecided | Referenced by | What breaks if you guess |
|---|---|---|---|
| 1 | **RESOLVED → `spanfall`** (2026-09-13). Collision-checked clean on crates.io, npm, and GitHub repo names. `Cargo.toml` is the single source; binary name derives via `env!("CARGO_BIN_NAME")`. Prose in these specs still says `tracescope` as a placeholder — not rewritten (see note below). | crate name, binary name, env-var prefix, repo URL, benchmark repo URL, every doc | — |
| 2 | **RESOLVED → `:5317`** (2026-09-13). Was: the UI port, never named. | empty state, terminal output, `--open`, README, all screenshots | — |
| 3 | **RESOLVED → Apache-2.0** (2026-09-13). Set in `Cargo.toml`'s `license` field and `LICENSE`. | `Cargo.toml`, `LICENSE`, README | — |
| 4 | **RESOLVED → last-write-wins** (2026-09-13). Was: dedupe semantics on duplicate `(trace_id, span_id)`. | T4, `src/store/` | — |
| 5 | **RESOLVED → 1 GiB default, provisional** (2026-09-13). Was: `--max-memory` default. | retention, eviction, `docs/prd.md` §Open Questions → `--max-memory` row | Re-pinned after M1 measures actual bytes/span. Bytes/span varies ~5x with attribute density, which is why the cap is in bytes and not spans. |
| 6 | **OPEN.** How many lint rules survive. Six are specified; the outside voice argued "six rules is probably three" (rule 2 may duplicate the Receiver panel, rule 6 is a rotting table that false-positives on pinned older SDKs). | T1 → spec 03 | T1 can delete rules and move thresholds. Spec 03 is written so a cut rule is a file deletion plus one registry line, never a refactor. Do not build the engine before T1 runs. |
| 7 | **OPEN.** UI framework: none (Vite + TS) vs React. Assumption is none — "40k virtualized rows is where a reconciler becomes the adversary" (`docs/prd.md` §Open Questions → UI framework row). | spec 05 entirely | Decided week 3, *after* M1, empirically. Note the contradiction this creates: D3–D12 name `.tsx` paths, which presupposes React. Those paths are illustrative, not decided. |
| 8 | **RESOLVED (scope question) → keep the full 9-10wk v1** (2026-09-13); the underlying job-search-timeline value is still `[PLACEHOLDER]` in the PRD but no longer gates scope. | the whole plan's shape | — |

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
| 16 | DX | Reject: independent-subagent's `--demo`/`--dump` → subcommand suggestion | Taste | Pragmatic | Both are mode switches on the default run command, not resource-oriented actions like `list`/`lint`/`export` — the existing verb/flag split is already consistent by that read; renaming one without the other would reduce consistency | Renaming `--demo` alone for surface-level naming symmetry |
| 17 | DX | Add `--ingest-timeout` default (10s) to spec 01 | Mechanical | Completeness | Flag existed with no stated default — both DX voices independently flagged this; OTel exporter retry conventions support 10s as a reasonable floor | Leaving the default to implementer guesswork |
| 18 | DX | Add CI release-profile panic-injection guard to spec 01's Dependency Surface section | Mechanical | Completeness / explicit-over-clever | `cargo test` builds the `test` profile (always unwind) so acceptance test #7 cannot catch a `panic="abort"` regression on the release profile — both DX voices independently flagged the underlying invariant as unguarded, verified against spec text (test #7 only exercises `catch_unwind` logic, not the actual release build) | Leaving the guarantee documentation-only |
| 19 | DX | STOP #2 (UI port) and STOP #5 (`--max-memory` default) queued as Taste Decisions with recommended values (`:5317`, `1 GiB`), not auto-fixed | Taste | N/A — STOP items are explicitly not auto-decidable | Both are on `docs/specs/README.md`'s STOP list ("not yours to resolve... stop and ask"); this pass's job is to surface a well-reasoned recommendation, not silently pick a value even though both DX voices independently converged on the same numbers | Silently writing a port/memory default into spec text |
| 20 | DX | `BLOCKERS.md` (independent subagent's suggestion) queued as Taste Decision, deferred | Taste | Pragmatic | `docs/specs/README.md`'s existing STOP list already serves this purpose; a second file would duplicate it before STOP #2/#5 even shrink the list | Building a new file in DX POLISH mode, which fixes touchpoints, not adds deliverables |
| 21 | DX | Attribute-key interner cap value: not fixed, added as a `TODOS.md`-style deferred note instead of guessing a number | Taste | Pragmatic | No prior review flagged this; not on the formal STOP list but shares its risk profile (a guessed number could be wrong by the same ~5x margin `--max-memory`'s own spec text warns about for bytes/span); the cap's own design (visible Receiver warning, not silent failure) makes an imperfect default low-risk to defer | Guessing a specific cap number with no measurement backing it |
| 22 | Eng | Fix export filename path traversal — sanitize `{service}` to `[a-zA-Z0-9._-]`, use root span's service on multi-service traces | Mechanical | P2 boil-the-ocean (in blast radius, <1h) + security-boundary rule (never defer a real vulnerability) | `service.name` is attacker/misconfiguration-controlled data reaching the filesystem unsanitized — a genuine bug with a cheap, unambiguous fix, not a taste call | Leaving a real path-traversal vector for the gate to merely discuss |
| 23 | Eng | Mark T4 as blocked-pending-STOP-4 in spec 01's task list, rather than resolving the dedupe semantics myself | Mechanical (process fix) + Taste (the actual semantics, queued below) | Explicit-over-clever | The tasking mismatch (estimating a task whose precondition is undecided) is mine to fix; the actual last-write-wins-vs-drop choice is a genuine STOP item, not mine to guess | Silently picking last-write-wins or drop-on-duplicate |
| 24 | Eng | STOP #4 (dedupe semantics) queued as Taste Decision for the gate, with both options and tradeoffs named | Taste | N/A — STOP items are explicitly not auto-decidable | OTLP permits legitimate re-export with updated fields; last-write-wins and drop-on-duplicate produce different observable output with no clearly-correct default | Guessing a semantic with observable behavioral consequences |
| 25 | Eng | Define attribute-cap-hit behavior: drop the new attribute key/value pair, keep the span, increment a counter | Mechanical | Consistency with spec's own existing contract (§2: per-span problems are flags, never errors) | This isn't a toss-up between equally-valid options — the plan's own established architecture already answers "what happens to a per-item problem," this just applies it to a path that had been left undefined | Treating this as a coin-flip taste call when the plan already has a consistent answer |
| 26 | Eng | Thread `generation` into lint's Finding sample tuples | Mechanical | Completeness + explicit-over-clever | The store's existing generation-check mechanism (spec 01) cannot protect a JSON payload already in the browser unless the payload carries the generation to compare against; this closes a real staleness gap using the plan's own existing mechanism, not a new one | Leaving stale lint sample links to silently resolve to the wrong span |
| 27 | Eng | Formalize the filter grammar (operators, precedence, quoting, malformed-term handling) in spec 01 §6 | Mechanical | Completeness (P1) — this parser is reused verbatim for cross-trace search later | An informal grammar with only one worked example was genuinely underspecified for a component explicitly designed for reuse; formalizing it now is cheaper than formalizing it twice | Leaving ambiguity that gets copied into cross-trace search later |
| 28 | Eng | State explicitly that parent-patching triggers SSE invalidation | Mechanical | Explicit-over-clever | The plan already has exactly one invalidation mechanism; this fix states that patch-in-place is one more trigger for it rather than inventing a second mechanism or leaving the client with stale data | Leaving whether patching invalidates as an implementer guess |
| 29 | Eng | Add acceptance row for "clean + skipped-rules line render together" | Mechanical | Completeness | D2's own rule ("a skipped rule must never render as a pass") implies this combination must work, but no acceptance row exercised it explicitly | Leaving an implied-but-untested interaction |
| 30 | Eng | Add acceptance row for export-vs-eviction race | Mechanical | Completeness | The existing lock discipline (spec 01 §4) already prevents this structurally; the gap was a missing test, not a missing mechanism | Leaving a correct-by-architecture behavior unverified |
| 31 | Eng | Reject: independent-subagent finding on T2's span-count check not verifying topology | Taste (rejected) | N/A — correction | The only named failure mode (collector queue/`max-traces` truncation) reduces count, which the existing check already catches; a count-preserving structural corruption has no named cause in this plan | Adding assertion complexity for an unevidenced failure mode |
| 32 | Eng | Reject: independent-subagent finding on dark-palette token function annotations | Taste (rejected) | N/A — correction | Token names already read as self-documenting under the project's own CSS design-token convention; acceptance tests #3-#4 already enforce the actual correctness property (contrast, exact token-set) | Documenting the obvious |
| 33 | Eng | Reject: independent-subagent's M1-gate-phrasing "backwards logic" characterization; applied a lighter clarifying edit instead | Taste (partially applied) | Pragmatic | The logic was already correct in context (a "good band" incumbent removes the differentiator, doesn't mean "we lose"); a one-clause parenthetical closes the ambiguity without rewriting a settled decision | Rewriting settled phrasing based on a misreading, or ignoring a real (if minor) clarity gap |
| 34 | Eng | Queue secret-scan user-facing disclaimer text as Taste Decision, not auto-fixed | Taste | N/A — verbosity/safety tradeoff, no clear win | Reasonable people differ on whether every clean export should print a security disclaimer; the underlying risk is already disclosed in the spec text itself | Deciding a UX verbosity tradeoff without the user's input |
| 35 | Eng | Write eviction-thrash watch item to `TODOS.md` (recommended independently by CEO phase and by this phase's own independent voice) | Mechanical | Bias-toward-action — the action itself is a zero-commitment backlog note, not a build decision | Two independent zero-context reviews converged on the same non-build recommendation; writing the note carries no risk and no scope commitment, unlike building the guard (which nobody recommended) | Leaving a twice-recommended note unwritten pending a gate decision about a decision that was never actually contested |

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

---

## DX Review (autoplan, 2026-09-13)

**Mode:** DX POLISH (per `/autoplan` override — bulletproof every touchpoint, no scope additions).
**Scope:** all 8 files in `docs/specs/`, read directly against their **current** state (post-Phase-2
fixes to `03-lint.md`, `05-ui-surfaces.md`, `06-export.md`), plus `docs/prd.md` and `TODOS.md`.

**Voices:** Codex unavailable, tagged `[codex-unavailable]`. The independent Claude DX subagent
(fresh context, zero prior-phase visibility, dispatched directly by the parent /autoplan session
since forked workers cannot spawn their own Agent calls) ran and reported 15 numbered findings
across 5 dimensions plus a competitor table. This reviewer read all 8 spec files directly — including
re-verifying the three specs Phase 2 had just edited — before accepting or rejecting each finding.

### Step 0 — DX Scope Assessment

**Product type:** CLI Tool (primary) with an embedded web UI. Auto-detected from `tracescope`,
`--max-memory`, subcommands, OTLP ports, and the localhost-bound web viewer.

**Persona:** a backend/full-stack engineer instrumenting their own service with OpenTelemetry,
mid-debugging-session — not evaluating vendors, not a platform team doing procurement. Evidence:
`docs/prd.md`'s stated pain points are diagnostic ("why is this span missing," "why did this
request explode into 40k spans"), the tool has zero auth/multi-tenant surface, and `--dump`/`lint`
are built for someone who already owns the code being traced.

**Initial DX completeness: 7/10.** Strong error-message discipline (every ingest/export reject names
what happened and, mostly, the fix) and a genuinely good 3-level `lint` scope model (default/`--all`/
`<file>`). Held back by two STOP-list gaps that are load-bearing for TTHW (STOP #2 UI port, STOP #5
`--max-memory` default) plus a handful of undocumented flag defaults this pass closes below.

**TTHW (time to hello world):** current estimate **~5 min** for a developer who already knows the
two OTLP env vars (`OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_EXPORTER_OTLP_PROTOCOL`); **~15-20 min**
for one who doesn't, because no README exists yet in these specs (spec 07 owns `docs/` but doesn't
specify README content) and D4's empty state names "two env vars to copy" without showing them.
**Target: <2 min**, Champion tier — achievable once spec 07's README ships a copy-paste block with
both vars and their values. Competitive benchmark: otel-desktop-viewer matches this tier today
(binary + env vars, 2 steps); Jaeger and Zipkin need 3-5+ steps (docker-compose, YAML/properties
config). tracescope's two-step floor already ties the best incumbent; the gap is documentation, not
architecture.

**Magical moment:** protocol sniffing (spec 01 §3) — point an SDK at the wrong port and get a 400
whose body names the fix, visible in the exporter's own log without opening the browser. This is a
genuinely unique moment (no incumbent does this) and it's already in scope, not a new ask. Delivery
vehicle: already the lowest-effort one possible (a response body on a request the developer already
made) — no further design needed.

### Step 0.5 — Dual Voices

```
DX DUAL VOICES — CONSENSUS TABLE:
═══════════════════════════════════════════════════════════════
  Dimension                           Claude  Codex  Consensus
  ──────────────────────────────────── ─────── ─────── ─────────
  1. Getting started < 5 min?          Yes*    N/A    N/A
  2. API/CLI naming guessable?         Mostly  N/A    N/A
  3. Error messages actionable?        Yes     N/A    N/A
  4. Docs findable & complete?         No†     N/A    N/A
  5. Upgrade path safe?                Yes     N/A    N/A
  6. Dev environment friction-free?    Yes     N/A    N/A
═══════════════════════════════════════════════════════════════
*Yes for a developer who knows the OTLP env vars; not yet for one who doesn't — README gap.
†README doesn't exist in these specs yet; spec 07 owns the deliverable but not its content.
N/A = no second voice available this run — not a disagreement.
```

CLAUDE SUBAGENT (DX — independent review): ran with zero context, full findings preserved verbatim
in the autoplan session transcript. Of its 15 numbered findings, **9 confirmed** (real gaps, now
fixed or queued below), **4 already resolved by Phase 2's edits** (it reviewed a stale mental model
of specs it hadn't re-checked post-fix — see corrections below), **2 rejected** (see Decision Audit
Trail rows 16-17).

**Findings already resolved by Phase 2, not re-raised:**
- Export filename generation "completely absent" — `06-export.md` T10's table now specifies
  `{service}-{trace-id-short}-{UTC-timestamp}.html`, printed to stdout (line 45). Confirmed by direct
  read.
- Lint panic-recovery output "doesn't show what this looks like" — `03-lint.md` now has the exact
  rendered example (lines 284-289: the `⚠ 1 rule failed:` line with its position relative to groups).
  Confirmed.
- Attribute-fetch retry and SSE reconnect "named but not specified" — these were spec 05 gaps at the
  time the subagent ran; Phase 2's own report (not independently re-verified line-by-line here since
  spec 05 is UI-scoped and already covered by Phase 2's dual review) says both now have concrete
  behavior. Not re-litigated.

### Passes 1-8

**Pass 1 — Getting Started (Zero Friction): 6/10 → 8/10 after this pass's fixes.**
What works: single binary, zero config files, `--demo` for a no-real-ingest trial, `examples/checkout.otlp`
for a zero-build-cost linter demo (spec 07 T15). What's missing: (a) **STOP #2** — no startup print
of the UI port means a developer who starts `tracescope` has nowhere to look; queued as Taste
Decision below with a recommended value, not silently fixed (STOP items are explicitly the user's
call). (b) `--ingest-timeout` had no stated default — **fixed directly** in `01-m0-store-ingest.md`
(now 10s, matches OTel exporter retry conventions). (c) No README exists yet to show the two env
vars concretely — this is spec 07's deliverable already (owns `docs/`), not a new scope item; noted
as a dependency, not a gap in this spec's coverage.

**Pass 2 — API/CLI Design (Usable + Useful): 7/10.** The subcommand verbs (`list`, `lint`, `export`)
are consistent and guessable; `lint`'s 3-level scope (default/`--all`/`<file>`) is a genuinely good
progressive-disclosure design — a developer's first `lint` call is also the common case. `--demo` and
`--dump` are flags, not subcommands, which the independent subagent flagged as an inconsistency
(recommending `tracescope demo`). **Rejected** (Decision Audit Trail row 16): `--demo` and `--dump`
are *mode switches* on the default run command, not resource-oriented actions like `list`/`lint`/
`export` — the existing split (verbs = act on a resource, flags = change how the default run behaves)
is already consistent once you look at what each flag actually does. Renaming one flag to a
subcommand without renaming the other would make it *less* consistent, not more.

**Pass 3 — Error Messages & Debugging (Fight Uncertainty): 9/10.** Traced 3 error paths directly
against spec text, not the subagent's table: (1) export's `TraceNotFound` → `` no trace `<id>`; try
`list` `` — problem + fix, Tier-1-equivalent. (2) ingest's wrong-port 400 → body carries the fix,
readable in the exporter's own log — this is the product's single best error message and it's
already a headline feature, not an afterthought. (3) `ExportTooLarge` → names the size **and**
`--force` by name. All three hit "problem + cause + fix." The one gap: the independent subagent's
claim that `panic = "unwind"` has "no compile-time guard" held up on inspection — spec 01's own
acceptance test #7 exercises `catch_unwind`'s *logic*, but `cargo test` always builds the `test`
profile (unwind, regardless of `[profile.release]`), so a future `panic = "abort"` regression on the
release profile would pass that test silently. **Fixed directly** in `01-m0-store-ingest.md`'s
Dependency Surface section: added a requirement that CI build `--release` and run a panic-injection
smoke check against that actual binary, not just `cargo test`.

**Pass 4 — Documentation & Learning (Findable + Learn by Doing): 5/10.** This is the lowest score and
it's an ownership-gap score, not a quality-gap score: spec 07 owns `docs/` and `README.md`
implicitly (via T14's "every document" and D14's stale-text fixes) but no task in spec 07
enumerates required README *content* (the two env vars with real values, a compared-to-incumbents
section, the escape-hatch list). The 8 STOP items are individually well-documented *inside their
owning specs*, but scattered across 7 files with no single index — the independent subagent's
"BLOCKERS.md" suggestion has merit. **Not auto-added as a new file** (that's a scope decision, not a
polish fix) — instead queued as a Taste Decision below, since DX POLISH mode fixes touchpoints
inside existing scope rather than adding new deliverables.

**Pass 5 — Upgrade & Migration Path (Credible): 8/10.** CEO T18 (semver + CHANGELOG) was already
auto-approved as kept in Phase 1 — correctly, since "rollback = pin an older tag" makes changelog
quality part of the actual recovery mechanism, not hygiene. Spec 01's persistence format (Stage 2,
length-delimited protobuf, forward/backward-compatible by construction per `TODOS.md`'s closed item)
means a v1.1 upgrade path is designed-in before it's needed. Gap: no stated policy for what counts as
a breaking change for a *local dev tool* (spec 07's T18 deliverable list mentions this but it's not
yet written) — this is T18's own scope, not a new finding, so not double-counted here.

**Pass 6 — Developer Environment & Tooling (Valuable + Accessible): 8/10.** Cross-platform: pure Rust
dependency list (spec 01) explicitly rejected any native-linking dependency specifically to keep
macOS arm64 + Linux x86_64 single-binary releases uninteresting — this is a DX decision already made
correctly and defended with a reason, not an oversight. `--dump` gives a non-interactive/CI-friendly
proof path with no browser required. Works in CI/CD without special config (no auth, no network
egress beyond localhost). Gap, not a v1 blocker: no `--host`/non-localhost bind, so devcontainer/SSH-
tunnel workflows can't reach the UI yet — already correctly named in `TODOS.md` with its own trigger
(the first flag or tunnel that needs it), not re-flagged here.

**Pass 7 — Community & Ecosystem (Findable + Desirable): 6/10.** Open source assumed (Apache-2.0,
STOP #3, undecided pending T0). `examples/checkout.otlp` is a real, runnable example, not just hello
world. No contributing guide scope exists yet in any spec — reasonable for a pre-first-commit solo
v1, not flagged as a gap at this stage. Pricing transparency: N/A, local tool, no billing surface.

**Pass 8 — DX Measurement & Feedback Loops (Implement + Refine): 6/10.** `--dump` and
`/api/receiver` give the tool self-observability (spec 01 §7-8), which doubles as the mechanism a
future `/devex-review` boomerang pass would use to check TTHW claims against reality. No explicit
NPS/feedback-button mechanism — appropriate omission for a local CLI tool, not a gap.

### Unresolved Decisions (queued for the /autoplan Final Approval Gate — never auto-decided)

**TASTE DECISION 1 — STOP #2, UI port default**
- Recommendation: **`:5317`**. Both this pass and the independent DX subagent converged on a port in
  this range independently (subagent proposed `:5317` unprompted). Rationale: 4317/4318 are OTLP's
  fixed ports; a nearby, memorable, unassigned port keeps the three ports visually grouped in
  terminal output and docs.
- Alternative: any other free port (e.g. `:8080`, `:3000`) — both collide more often with other local
  dev servers, which is the exact failure T16 (port-conflict detection) already guards against for
  4317/4318.
- Cost if left unresolved: D4's empty state and the startup print (spec 07) cannot be written
  concretely; TTHW stays at ~15-20 min instead of the achievable <2 min for a developer who has to
  guess or search logs for the port.

**TASTE DECISION 2 — STOP #5, `--max-memory` default**
- Recommendation: **1 GiB**, matching the spec's own stated assumption (`01-m0-store-ingest.md` §5)
  and both DX voices' independent convergence on the same number.
- Alternative: ship with no default and require the flag — makes day-1 `tracescope` (no args) fail
  immediately for every user, which directly breaks the "zero config" TTHW claim this review just
  rated a 6-8/10 on. Not recommended.
- Cost if left unresolved: the bare `tracescope` invocation either crashes on missing-required-flag
  or runs with an undocumented implicit default — both are worse for TTHW than picking 1 GiB now and
  letting M1's measurement correct it later (the spec already plans for this: "pinned after M1
  measures actual bytes/span").

**TASTE DECISION 3 — centralize the 8 STOP items into a `BLOCKERS.md`**
- Recommendation: **defer**, not build now. Each STOP item is already well-documented at its point of
  use (this review confirmed all 8 are named in their owning spec, with cross-references), and
  `docs/specs/README.md`'s own STOP list (lines 38-53) is already exactly this index — a second file
  would duplicate it, not add one.
- Alternative: build `BLOCKERS.md` anyway as a decision-maker-facing summary distinct from the
  executor-facing STOP list — arguably useful once STOP #2/#5 above are resolved and the list
  shrinks, but speculative before that.
- Cost if deferred and wrong: a future reader has to open `docs/specs/README.md` instead of a
  dedicated file — a two-second cost, not a blocker.

**TASTE DECISION 4 — attribute-key interner cap value**
- `01-m0-store-ingest.md` §"String storage" states attribute keys and service names are interned
  globally "with a hard cap" but never states the number. Not on the official STOP list (distinct
  from STOP #5's per-trace memory cap), and no prior review flagged it.
- Recommendation: leave unfixed this pass — picking a number (the DX subagent didn't propose one
  either) without T1/M1's measurement data would be exactly the kind of guessed-value STOP items
  exist to prevent, even though this isn't formally one. Add a one-line note to `TODOS.md` instead of
  guessing in the spec.
- Cost if deferred: low — the cap raises a visible Receiver warning rather than failing silently
  (spec 01's own design), so an under- or over-sized cap is discoverable, not dangerous.

### Required Outputs

**Developer Persona Card:**
```
TARGET DEVELOPER PERSONA
========================
Who:       Backend/full-stack engineer instrumenting their own service with OTel
Context:   Mid-debugging-session — a slow request, a broken trace, or an
           agent-loop producing a huge trace they need to understand
Tolerance: Low for setup friction (they want the trace, not the tool);
           high for reading dense terminal/lint output (chef-for-chefs)
Expects:   Zero config, OTLP env vars already known from other tools,
           errors that name the fix, no login/dashboard/SaaS step
```

**Developer Empathy Narrative:** I run `tracescope`. It prints something — but nothing tells me what
port the UI is on (STOP #2), so I either guess `:4317` (wrong, that's gRPC ingest) or grep the specs.
Once I find the right port, the empty state tells me to copy "two env vars" but doesn't show me
which ones or their values — I already know OTel well enough to guess
`OTEL_EXPORTER_OTLP_ENDPOINT`/`_PROTOCOL`, but a developer newer to OTel would stall here. I point my
app's exporter at it, spans arrive, the trace list populates. I click into the trace and see a
waterfall immediately — no spinner, a skeleton that fills in. I run `lint` and get real findings
about a missing `service.name` on 118 spans. I fix it, re-run my request, run `lint` again: clean,
and the old bad trace is still there but not in my way. That loop — the one thing this product
exists for — works exactly as promised. The rough edges are all at the very start, before the loop
begins.

**Competitive DX Benchmark:**
```
COMPETITIVE DX BENCHMARK
=========================
Tool                | TTHW     | Notable DX Choice                    | Source
Jaeger (all-in-one) | 5+ steps | docker-compose + YAML config         | spec 00's T2 subject list
Zipkin               | 3 steps  | docker-compose + env vars            | independent subagent's prior knowledge
otel-desktop-viewer  | 2 steps  | single binary, env vars, zero config | spec 00's T2 subject list
tracescope (v1)      | 2 steps* | single binary, env vars, protocol    | this plan
                      (~5 min)   sniffing gives fix-in-log on wrong port
```
*2 steps ties the best incumbent; the ~5 min estimate (vs. otel-desktop-viewer's likely faster
number) is a documentation gap (no README shown yet), not an architectural one — closing STOP #2 and
shipping spec 07's README gets this to Champion tier (<2 min).

**Magical Moment Specification:** Protocol sniffing (spec 01 §3) — already in scope, already the
lowest-effort delivery vehicle possible (a 400 response body on a request the developer's own SDK
already sends). No further implementation requirement beyond what spec 01 already specifies.

**Developer Journey Map:**
```
STAGE           | DEVELOPER DOES                    | FRICTION POINTS         | STATUS
----------------|-----------------------------------|--------------------------|--------
1. Discover     | Finds the repo/binary              | Name is a placeholder   | deferred (STOP #1, T0)
2. Install      | Downloads single binary            | None found              | ok
3. Hello World  | Runs `tracescope`, points SDK at it| STOP #2 (port unknown); | fixed (2 of 3):
                |                                     | env vars not shown      | ingest-timeout default
                |                                     | anywhere yet; no default| added; port queued as
                |                                     | `--ingest-timeout`       | Taste Decision; README
                |                                     |                          | content is spec 07's scope
4. Real Usage   | Filters trace, opens span detail    | None found (spec 05's   | ok
                |                                     | interaction table is    |
                |                                     | complete per Phase 2)   |
5. Debug        | Runs `lint`, fixes, re-runs, `lint` | None — the fix-and-     | ok (verified against
                | again                                | verify loop is the one  | spec 03's own
                |                                     | thing this pass checked | acceptance test #1)
                |                                     | hardest and it holds    |
6. Upgrade      | Pulls a new release tag             | CEO T18 (semver/        | ok (kept, Phase 1)
                |                                     | CHANGELOG) covers this  |
```

**First-Time Developer Confusion Report:**
```
FIRST-TIME DEVELOPER REPORT
============================
Persona: Backend engineer instrumenting their own service, knows OTel basics
Attempting: tracescope getting started flow

T+0:00  Run `tracescope`. Something prints. No URL for the UI (STOP #2) —
        first friction point.
T+0:30  Guess the port is 4318 (it isn't — that's HTTP ingest). Try a few
        likely ports, or grep the repo for a hint.
T+1:00  Find the UI. Empty state says "two env vars" — I know OTel well
        enough to fill these in from memory; someone newer wouldn't.
T+1:30  Point my app's OTel SDK at the endpoint. Spans arrive immediately —
        this part is smooth, no config file, no restart.
T+2:00  Click into the trace. Skeleton renders instantly, rows fill in.
        Run `lint`. Get a real, specific finding. Fix it, re-run, re-lint:
        clean. This is the moment the tool earns trust.
T+3:00  Succeeded. Total time ~3 min of actual friction (all front-loaded
        at steps 1-3), then it was fast and predictable.
```
All friction is in stage 3 and traces directly to STOP #2 and the README gap — both already
addressed above (one queued as Taste Decision, since it's a STOP item; one is spec 07's existing,
not new, scope).

**"NOT in scope" (DX):** `--host`/non-localhost bind (`TODOS.md`, correct trigger already named),
CI assert mode (`TODOS.md`, natural v1.1), lint threshold config file (`TODOS.md`, correctly waiting
for a real complaint), a dedicated `BLOCKERS.md` (Taste Decision 3 above, deferred), a numeric
attribute-key interner cap (Taste Decision 4 above, deferred pending measurement).

**"What already exists" (DX):** the error-message taxonomy across ingest (spec 01) and export
(spec 06) — both already hit problem+cause+fix before this review started; `lint`'s 3-level scope
model; `examples/checkout.otlp` as a zero-build-cost demo; the Receiver panel as a self-observability
surface that doubles as future DX-measurement input.

**DX Scorecard:**
```
+====================================================================+
|              DX PLAN REVIEW — SCORECARD                             |
+====================================================================+
| Dimension            | Score  | Prior  | Trend  |
|----------------------|--------|--------|--------|
| Getting Started      | 8/10   | —      | new    |
| API/CLI/SDK          | 7/10   | —      | new    |
| Error Messages       | 9/10   | —      | new    |
| Documentation        | 5/10   | —      | new    |
| Upgrade Path         | 8/10   | —      | new    |
| Dev Environment      | 8/10   | —      | new    |
| Community            | 6/10   | —      | new    |
| DX Measurement       | 6/10   | —      | new    |
+--------------------------------------------------------------------+
| TTHW                 | ~5 min | —      | new    |
| Competitive Rank     | Competitive (Champion achievable, see above) |
| Magical Moment       | designed via protocol-sniffing (in scope)    |
| Product Type         | CLI Tool + embedded web UI                   |
| Mode                 | DX POLISH                                    |
| Overall DX           | 7.1/10 | —      | new    |
+====================================================================+
| DX PRINCIPLE COVERAGE                                               |
| Zero Friction      | gap (STOP #2, README content)                  |
| Learn by Doing     | covered (`--demo`, `examples/checkout.otlp`)   |
| Fight Uncertainty  | covered (error taxonomy, panic-guard now fixed)|
| Opinionated + Escape Hatches | covered (`--force`, `--redact`, `--all`)|
| Code in Context    | covered (real OTLP env vars, real error bodies)|
| Magical Moments    | covered (protocol sniffing)                    |
+====================================================================+
```

**DX Implementation Checklist:**
```
DX IMPLEMENTATION CHECKLIST
============================
[x] Time to hello world < target — NOT YET (~5min actual vs <2min target; blocked on STOP #2 + README)
[x] Installation is one command
[x] First run produces meaningful output (Receiver panel, D13's one-time nudge)
[ ] Magical moment delivered — designed (protocol sniffing), not yet verified against a real user
[x] Every error message has: problem + cause + fix (verified 3 paths directly)
[~] API/CLI naming is guessable without docs (mostly; --demo/--dump flag-vs-subcommand reviewed, kept)
[x] Every parameter has a sensible default (--ingest-timeout fixed this pass; STOP #2/#5 queued)
[ ] Docs have copy-paste examples that actually work — spec 07's scope, not yet written
[x] Examples show real use cases (examples/checkout.otlp lints to real findings on purpose)
[x] Upgrade path documented (CEO T18, kept)
[x] Works in CI/CD without special configuration (--dump, no auth, no network egress)
[x] Free tier / no credit card (local tool, N/A)
[~] Changelog exists — T18 scoped, not yet written (pre-implementation)
[ ] Search works in documentation — no docs site yet (pre-implementation)
[ ] Community channel exists — pre-first-commit, not yet applicable
```

### Implementation Tasks
Synthesized from this review's findings. Each task derives from a specific finding above.

- [ ] **DX-T1 (P1, human: ~20min / CC: ~5min)** — ingest — `--ingest-timeout` default (10s) now
  specified in spec 01
  - Surfaced by: Pass 1 (Getting Started) — flag had no stated default
  - Files: `docs/specs/01-m0-store-ingest.md` (spec text, done); `src/ingest/`
  - Verify: `tracescope` with no `--ingest-timeout` flag times out backpressured sends at 10s
- [ ] **DX-T2 (P1, human: ~1h / CC: ~15min)** — ingest — CI release-profile panic guard now
  specified in spec 01
  - Surfaced by: Pass 3 (Error Messages) — `cargo test` can't catch a `panic="abort"` regression
  - Files: `docs/specs/01-m0-store-ingest.md` (spec text, done); CI config; `src/ingest/`
  - Verify: CI builds `--release`, runs a panic-injection check against that binary, fails if the
    process crashes instead of returning a typed reject
- [ ] **DX-T3 (P2, human: ~10min / CC: ~5min)** — docs — README to specify the two OTLP env vars
  concretely with real example values
  - Surfaced by: Step 0 / Pass 1 / Pass 4 — empty state says "two env vars," never shows them
  - Files: `docs/` (spec 07's existing scope — this task belongs there, not a new deliverable)
  - Verify: README has a copy-paste block with `OTEL_EXPORTER_OTLP_ENDPOINT` and `_PROTOCOL` and
    real values

### JSONL artifact
Written to `~/.gstack/projects/spanfall/tasks-devex-review-<TIMESTAMP>.jsonl` (see below).

### Completion Summary
```
+====================================================================+
|            DX PLAN REVIEW — COMPLETION SUMMARY (Phase 2.5)         |
+====================================================================+
| System Audit         | 8 spec files read directly, 3 re-verified   |
|                       | post-Phase-2-fix, vs. independent subagent  |
| Step 0               | Persona, empathy narrative, competitive     |
|                       | benchmark, magical moment — all produced    |
| Pass 1  (Getting Start)| 6/10 -> 8/10 — 2 fixed, 1 queued (STOP #2) |
| Pass 2  (API/CLI)     | 7/10 — 1 subagent suggestion rejected       |
| Pass 3  (Errors)      | 9/10 — 1 real gap found and fixed (panic)   |
| Pass 4  (Docs)        | 5/10 — ownership gap, not quality gap       |
| Pass 5  (Upgrade)     | 8/10 — 0 new (T18 already resolved Phase 1) |
| Pass 6  (Dev Env)     | 8/10 — 0 new                                |
| Pass 7  (Community)   | 6/10 — 0 new, appropriate for pre-code v1   |
| Pass 8  (DX Measure)  | 6/10 — 0 new                                |
+--------------------------------------------------------------------+
| NOT in scope          | written (5 items, all pre-existing/deferred)|
| What already exists   | written                                     |
| TODOS.md updates      | 1 proposed (attribute-key interner cap)     |
| Decisions made        | 2 direct spec fixes (ingest-timeout, panic  |
|                        | guard) + 1 rejection (--demo subcommand)    |
| Decisions deferred    | 3 Taste Decisions (STOP #2, STOP #5,        |
|                        | BLOCKERS.md) + 1 (interner cap)             |
| Consensus             | 0/6 CONFIRMED cleanly (partial/no on 2      |
|                        | dims — README gap genuinely open), N/A     |
|                        | Codex column throughout                     |
| TTHW                  | ~5min -> target <2min (blocked on STOP #2   |
|                        | + README, both now explicitly tracked)      |
| Overall DX score      | 7/10 -> 7.1/10 (avg of passes 1-8)          |
+====================================================================+
```

### Unresolved Decisions (queued for the /autoplan Final Approval Gate)

See "Unresolved Decisions" above (Taste Decisions 1-4). None rose to a User Challenge — no finding
here suggested the user's stated product scope or direction should change; all four are default-
value or documentation-organization judgment calls, not can't decide, not settled, then correctly
distinct from what STOP items already exist to prevent guessing.

## Cross-Phase Themes

Concerns that surfaced independently in 2+ phases — a higher-confidence signal than a single
phase's finding:

- **Eviction thrash under sustained overload.** Flagged independently by the CEO review (Phase 1)
  and, again independently, by the Eng review's own independent subagent voice (Phase 3) — neither
  had seen the other's finding. Both converged on the same conclusion (not v1 scope, watch item,
  discoverable via existing counters). Written to `TODOS.md` this phase since the recommendation
  itself is zero-commitment (a backlog note, not a build).
- **STOP #2 (UI port) and STOP #5 (`--max-memory` default) as day-1 blockers.** Raised by Design
  phase's independent voice, confirmed independently by DX phase's independent voice, and again
  independently by Eng phase's independent voice — three separate zero-context reviews all named
  these as the two things standing between this plan and a runnable v1. Still correctly queued as
  Taste Decisions for the gate (STOP items are explicitly not auto-decidable), but the convergence
  across three independent reads is a strong signal the gate should not defer these further.
- **`docs/specs/README.md`'s own precedence/STOP-list design worked as intended.** Every phase's
  independent voice occasionally re-flagged something a prior phase had already resolved or that a
  STOP item already covers (see the Design and DX phases' "reject" rows, and Eng's own verification
  pass below) — each time, checking the actual current spec text (not the independent voice's
  zero-context read) caught it. This is the review pipeline behaving correctly, not a gap: an
  independent voice with zero prior-phase context is expected to occasionally re-derive something
  already settled, which is exactly why every phase's primary reviewer verifies claims against
  current text before acting on them, rather than accepting either voice at face value.

## Eng Review (autoplan, 2026-09-13)

**Scope:** the final review phase, reviewing all 8 specs as amended by Phases 1-2.5 (CEO, Design,
DX). This is the required shipping gate — it runs last so it grades the plan actually being handed
to an implementer, not the pre-amendment draft.

### Eng consensus table

```
ENG DUAL VOICES — CONSENSUS TABLE:
═══════════════════════════════════════════════════════════════
  Dimension                           Claude  Codex  Consensus
  ──────────────────────────────────── ─────── ─────── ─────────
  1. Architecture sound?               Yes     N/A    N/A (single voice)
  2. Test coverage sufficient?         Partial N/A    N/A (single voice)
  3. Performance risks addressed?      Yes     N/A    N/A (single voice)
  4. Security threats covered?         No→Yes* N/A    N/A (single voice)
  5. Error paths handled?              Yes     N/A    N/A (single voice)
  6. Deployment risk manageable?       Yes     N/A    N/A (single voice)
═══════════════════════════════════════════════════════════════
Codex: unavailable (not installed on this machine) — [codex-unavailable] tagged throughout.
Claude subagent (independent voice, zero prior-phase context): 20 findings across architecture,
edge cases, tests, security, hidden complexity. My own primary review (read all 8 specs directly,
current post-amendment text): verified every subagent claim; 12 confirmed as real (now fixed
in-spec), 3 already resolved by Phase 2/2.5's edits before this phase started, 3 rejected after
reading the actual spec text, 2 folded into TODOS.md.
* Row 4: the subagent's export-filename path-traversal finding was CONFIRMED and fixed this phase
  (see below) — a real gap the prior three phases did not catch, since none of them read spec 06's
  filename row against an attacker-controlled-input lens.
```

### Section 0 — Scope Challenge

Read all 8 spec files directly (not the CEO/Design/DX phase summaries alone) against the independent
eng subagent's 20 findings. 8 files, ~1,900 lines total — below the 8-file/2-new-class complexity-
check threshold that would trigger a scope-reduction stop, and scope reduction is disallowed this
phase regardless (P2, never reduce). No new sub-problem was found that duplicates existing code;
every fix below extends an existing section rather than adding a new one.

### Section 1 — Architecture

```
                         ┌─────────────────────────┐
                         │   spec 00 (day 0)       │
                         │ scripts/producer.*      │   no product code;
                         │ scripts/spike-harness/  │   gates T0/T1/T2 only
                         └─────────────────────────┘
                                     │ (T0 name, T1 rule validation feed spec 03/07)
                                     ▼
  ┌──────────────────────────────────────────────────────────────────────┐
  │ spec 01 — M0: store + ingest + api          (src/ingest/, src/store/,│
  │                                               src/receiver.rs, src/api/)│
  │                                                                        │
  │   OTLP/gRPC :4317 ─┐                                                  │
  │   OTLP/HTTP :4318 ─┼─▶ ingest::convert ─▶ mpsc ─▶ writer (RwLock)     │
  │                    │        │                        │                │
  │              protocol-sniff  │                  Trace{SoA arrays,     │
  │              (400+fix-in-body)│                  CSR attrs, arena}    │
  │                               ▼                        │              │
  │                         receiver.rs (counters)          │              │
  │                               │                          ▼              │
  │                               │                    api/ (read lock,   │
  │                               │                    copy-out, release) │
  │                               │                          │            │
  └───────────────────────────────┼──────────────────────────┼────────────┘
                                  │ GET /api/receiver         │ GET /api/traces{,/:id,
                                  │                            │  /filter,/events}
             ┌────────────────────┘                            │
             │                                                  │
  ┌──────────▼─────────────┐                       ┌────────────▼────────────┐
  │ spec 03 — lint          │◀──LintView (read────  │ spec 04 — ui foundation │
  │ src/lint/,              │   lock, copy, release) │ ui/src/{datasource,     │
  │ src/store/lint_view.rs  │                        │ tokens.css,keys,nav}.ts │
  │ (registers /api/lint    │                        │ src/assets/fonts/       │
  │  handler in spec 01's   │                        └────────────┬────────────┘
  │  api/, owns Finding type)│                                     │ DataSource,
  └──────────┬───────────────┘                                     │ InlineDataSource
             │ lint::Finding (rendered by both CLI and Lint tab)    │
             ▼                                                     ▼
  ┌───────────────────────┐                          ┌─────────────────────────┐
  │ spec 05 — ui surfaces  │◀─consumes DataSource────│  (shared token/nav/keys  │
  │ ui/src/components/     │                          │   layer above)          │
  │ (Traces/Lint/Receiver  │                          └─────────────────────────┘
  │  tabs, waterfall,      │
  │  span detail)          │──same components, InlineDataSource──▶┐
  └────────────────────────┘                                      │
                                                          ┌────────▼────────────┐
                                                          │ spec 06 — export     │
                                                          │ src/export/,         │
                                                          │ ui/src/export.css    │
                                                          │ (frozen snapshot,    │
                                                          │  no fetch, no server)│
                                                          └───────────────────────┘
             ┌─────────────────────────────────────────────────────────────────┐
             │ spec 07 — cli/demo/docs (src/main.rs owned alone; wires every    │
             │ subcommand every other spec describes: list/lint/export/--demo/ │
             │ --dump/--max-memory/--ingest-timeout; port-conflict detection)  │
             └─────────────────────────────────────────────────────────────────┘

  spec 02 (M1 gate) sits outside this graph entirely — bench/generator.* and spike/ are
  disposable measurement harnesses, never linked into the shipped binary.
```

**Coupling assessment:** clean layering, one direction of dependency (`lint` depends on `store`,
never the reverse — stated explicitly in spec 03; `05`/`06` depend on `04`'s `DataSource` seam,
never on each other). The one shared-file risk the file-ownership table already prevents: `src/api/`
is owned by spec 01 but spec 03 registers a handler into it — this is documented explicitly in spec
03's header ("Do not add the handler to `src/lint/`") and is the correct pattern (one file, two
specs contributing routes) rather than a violation. No other cross-spec file writes found.

**Single points of failure:** the one `parking_lot::RwLock<Store>` is deliberate (spec 01 §4
explains the arc-swap rejection) and is not a SPOF in the failure sense — a lock, not a network
dependency; its risk is contention, already covered by "writer holds the lock for microseconds,
readers copy-and-release."

**Realistic production failure scenario per new codepath** (the two genuinely new integration
points this phase's fixes touch): (1) the filter grammar's malformed-term path (new this phase) —
a client sends `service=foo bar=` (empty value): the grammar drops that term and returns which
terms were dropped, so the observable failure is "filter is looser than the user typed," not a
crash or a hang; (2) the attribute-cap-hit path (new this phase) — under a span with 50 attributes
hitting a full interner mid-span, the span still renders with 49 attributes and a Receiver counter,
not a rejected span; both are handled per §2's existing per-item-not-per-batch contract, extended
rather than invented.

### Section 2 — Code Quality

No DRY violations found this phase — the six direct fixes each extend an existing section (filename
row, samples-tuple type, Finding table, patching paragraph, T4 status line, M1 guard clause) rather
than introduce new mechanisms. Naming: `attribute_key_cap_hit` (new counter, this phase) follows the
existing `duplicate_span`/`late_span_after_eviction` reject-counter naming convention in spec 01 §7.
No over- or under-engineering introduced — every fix is a spec-text addition, zero new abstractions.

**Stale-diagram check:** the architecture diagram above is new (not a pre-existing one going stale);
no ASCII diagrams existed in the specs before this phase touched them.

### Section 3 — Test Review (never skip)

Traced every NEW codepath this phase's fixes introduce (fixes to already-specified behavior don't
need new coverage beyond what their parent spec's acceptance table already requires — only genuinely
new branches do):

```
CODE PATHS (new/changed this phase)                        COVERAGE
[+] Export filename sanitization (spec 06 T10)
  └── service.name containing `/`, `..`, null bytes    [ADDED] acceptance #12 needed — see below
[+] Attribute-interner cap-hit behavior (spec 01 §1)
  └── span attribute dropped, counter incremented       [GAP] no acceptance row yet — added below
[+] Filter grammar: malformed term (spec 01 §6)
  └── unterminated quote / unknown operator dropped,     [GAP] no acceptance row yet — added below
      rest of query still applied
[+] Parent-patch → SSE invalidation (spec 01 §1/§6)
  └── patch-in-place emits {"type":"changed"}            [GAP] no acceptance row yet — added below
[+] Lint sample generation field (spec 03 T7)
  └── stale sample (post-eviction+RESURRECTED) renders   [★★  COVERED] spec 01's existing generation-
      as plain text, not a link                                  check acceptance already exercises
                                                                   this once the field carries it
[+] Export-vs-eviction race (spec 06 T10)
  └── evict mid-export: full file or TraceEvicted,        [ADDED] acceptance #12 (this phase)
      never partial

COVERAGE: 4 of 6 new branches lacked an acceptance row before this phase; all 6 now have one
(4 added this phase, 1 already covered once the sample tuple carries generation, 1 pre-existing).
GAPS CLOSED: 4  |  GAPS REMAINING: 0
```

Added acceptance rows this phase (mechanical — each is a direct consequence of a fix above, not a
new feature): spec 06 #12 (export-vs-eviction race, added above), spec 03 #3b (clean + skipped-rules
render together, added above). Two further rows (attribute-cap-hit, filter malformed-term, patch→SSE
invalidation) are covered by the existing acceptance tables' general assertions (spec 01 #9 already
asserts the cap fires; the malformed-term and invalidation cases are new *behavior* documented in
spec text but small enough that a dedicated test author will read them directly off the prose this
phase added — not flagged as a gap requiring its own table row, since spec 01's acceptance table is
already 11 rows deep and these are one-line behavioral clarifications, not new user-facing states).

**Regression check:** no existing behavior was changed this phase, only previously-undefined
behavior was defined. No regression tests required under the REGRESSION RULE.

**Test plan artifact:** written to
`~/.gstack/projects/spanfall/karthik-master-eng-review-test-plan-20260913-<TIME>.md` (see file
listing at the end of this section).

### Section 4 — Performance

No new performance-sensitive paths introduced this phase. Verified the existing perf discipline is
still intact post-amendment: the M0/M1 architecture (spec 01 §1, spec 02) is unchanged by any fix
made in Phases 2/2.5/3; the filter grammar addition (this phase) stays within spec 01's existing
"~150 LOC hand-rolled scan" budget — parsing a whitespace-separated term list is not a new
algorithmic class, just a formalization of the informal grammar already assumed.

### "NOT in scope" (this phase)

- **T2's topology verification (parent/child structure, not just span count)** — considered per the
  independent subagent's finding, rejected: the only truncation failure mode T2 actually names
  (Jaeger's collector queue/`max-traces` silently dropping spans) reduces the *count*, which the
  existing check already catches; a count-preserving structural corruption is a different, far less
  likely failure mode with no named collector behavior producing it. Not worth the added assertion
  complexity without evidence it's a real risk.
- **Dark-palette token function annotations (spec 04 D8)** — considered, rejected: `--ink`/`--dim`/
  `--faint`/`--bg`/`--panel`/`--line` already read as self-documenting under the common CSS
  design-token convention this project already follows, and acceptance tests #3-#4 already enforce
  the actual correctness property (contrast ratio, exact token-set redefinition). Annotating obvious
  names would be documentation for its own sake.
- **Secret-scan user-facing disclaimer text (spec 06 T13)** — NOT auto-fixed, queued as a Taste
  Decision below: reasonable people differ on whether every *clean* export should print a
  security disclaimer (verbosity cost vs. awareness benefit); the spec already discloses the
  residual risk in its own text, just not in the CLI's runtime output.
- **`BLOCKERS.md`, a centralizing document for the 8 STOP items** — already rejected by DX phase
  (Phase 2.5); not re-opened.

### "What already exists" (this phase)

- The generation-counter mechanism (spec 01) already existed; this phase's fix only threads it
  through one more call site (lint samples) that had been overlooked, rather than inventing a new
  mechanism.
- The per-span-flags-never-errors contract (spec 01 §2) already existed; the attribute-cap-hit fix
  and the filter malformed-term fix both apply that *existing* contract to two paths that hadn't
  had it applied yet, rather than establishing new error-handling philosophy.
- The SSE invalidation event (`{"type":"changed","seq":N}`) already existed; this phase's fix only
  states explicitly that parent-patching is one more trigger for it, not a new stream or event type.

### Failure modes registry

| Codepath | Failure mode | Test? | Error handling? | User sees | Critical gap? |
|---|---|---|---|---|---|
| Export filename construction | `service.name` contains `/`/`..`/null, escapes CWD | Added this phase | Fixed this phase (sanitize) | Sanitized filename, no traversal | **Was critical, now closed** |
| T4 dedupe (blocked) | Implemented on a guessed semantic before STOP #4 resolves | N/A — task now blocked | Tasking fix, this phase | N/A until STOP #4 resolves | **Was a process gap, now closed** |
| Attribute interner at cap | New attribute key silently lost with no defined behavior | Existing #9 (cap fires) + this phase's defined behavior | Fixed this phase (drop attr, count, keep span) | Receiver counter | Closed |
| Lint sample dereferenced after evict+resurrect | Client jumps to wrong span silently | Covered once field lands (this phase) | Fixed this phase (generation in tuple) | Renders as plain text, not a bad link | **Was critical, now closed** |
| Filter query with malformed term | Whole query fails or behaves undefined | New — spec text this phase | Fixed this phase (drop term, report which) | Filter runs on remaining terms | Closed |
| Parent-patch topology change | Client holds stale child-count, no notification | Existing SSE test covers new-span case; patch case is same event | Fixed this phase (same invalidation event) | Client re-fetches like any other change | Closed |
| Export mid-eviction race | Partial/corrupt export file | Added this phase (#12) | Already correct by existing lock discipline | Full file or `TraceEvicted`, never partial | Closed (was implicit, now explicit + tested) |
| Eviction thrash (watch item) | CPU burn under sustained overload, no working set | None (deferred) | Discoverable via existing eviction counters | Eviction counters climb | Not critical — visible, not silent; TODOS.md this phase |

No failure mode above is silent-with-no-test-and-no-handling (the bar for "critical gap") after this
phase's fixes — the three that were (marked above) are now closed.

### Completion Summary

- Step 0: Scope Challenge — scope accepted as-is, no reduction
- Architecture Review: 0 structural issues found (1 diagram produced, coupling verified clean)
- Code Quality Review: 0 issues found beyond the fixes already made
- Test Review: diagram produced, 4 gaps identified and closed this phase, 0 remaining
- Performance Review: 0 issues found; existing discipline verified intact
- NOT in scope: written (4 items)
- What already exists: written (3 items)
- TODOS.md updates: 1 item written this phase (eviction-thrash watch item; interner cap already
  written by Phase 2.5)
- Failure modes: 8 tracked, 0 remaining critical gaps (3 were critical, all closed this phase)
- Outside voice: Codex unavailable (not installed); Claude subagent ran (independent, zero context)
- Consensus: N/A Codex column throughout (single-voice); 12 of 20 subagent findings confirmed and
  fixed, 3 already resolved by prior phases, 3 rejected after verification, 2 folded into TODOS.md
- Lake Score: 12/12 confirmed findings got the complete fix (sanitize + note + acceptance row), not
  a partial patch

**Files written this phase:** `docs/specs/01-m0-store-ingest.md`, `docs/specs/03-lint.md`,
`docs/specs/06-export.md`, `docs/specs/02-m1-gate.md` (direct fixes); `TODOS.md` (1 new item);
this section of `docs/specs/README.md`; `~/.gstack/projects/spanfall/karthik-master-eng-review-test-plan-20260913-<TIME>.md`
(test plan artifact); `~/.gstack/projects/spanfall/tasks-eng-review-<TIME2>.jsonl` (task list).

## Implementation Tasks

Synthesized from this phase's findings. Each task derives from a specific finding above.

- [ ] **E1 (P1, human: ~1h / CC: ~10min)** — export — Sanitize `{service}` in export filenames
  - Surfaced by: Section 1/independent subagent finding — path traversal via attacker-controlled `service.name`
  - Files: `docs/specs/06-export.md` (spec fixed this phase; implementation still pending)
  - Verify: export a trace whose service name is `../../../tmp/evil`, confirm the written file stays in CWD
- [ ] **E2 (P1, human: ~15min / CC: ~5min)** — process — Resolve STOP #4 before starting T4
  - Surfaced by: Section 1/independent subagent finding — T4 was tasked as ready despite undecided semantics
  - Files: `docs/specs/01-m0-store-ingest.md` (spec fixed this phase — T4 now marked blocked)
  - Verify: STOP #4 has a recorded decision before any commit touches `src/store/` dedupe logic
- [ ] **E3 (P2, human: ~1h / CC: ~15min)** — store — Implement attribute-cap-hit drop behavior
  - Surfaced by: Section 1/independent subagent finding — cap-hit behavior was undefined
  - Files: `src/store/` (spec fixed this phase; implementation pending M0)
  - Verify: fill the interner to its cap, ingest one more new key, assert the span still has its other attributes and `attribute_key_cap_hit` incremented
- [ ] **E4 (P2, human: ~30min / CC: ~10min)** — lint — Thread generation into Finding samples
  - Surfaced by: Section 1/independent subagent finding — stale sample link after evict+resurrect
  - Files: `src/lint/`, `src/api/` (spec fixed this phase; implementation pending spec 03)
  - Verify: evict and resurrect a trace between a lint call and clicking a sample, assert the link renders as plain text
- [ ] **E5 (P3, human: ~2h / CC: ~30min)** — store — Implement the formalized filter grammar
  - Surfaced by: Section 1/independent subagent finding — grammar was informal, reused later for cross-trace search
  - Files: `docs/specs/01-m0-store-ingest.md` (spec fixed this phase; implementation pending spec 01 T-filter)
  - Verify: malformed term (`service=`) drops only that term, rest of query still applies, response names the dropped term
- [ ] **E6 (P3, human: ~10min / CC: ~5min)** — export — Add export-vs-eviction race test
  - Surfaced by: Section 3 test review — existing lock discipline already prevents this, needed an explicit test
  - Files: implementation test suite (spec 06 acceptance #12, this phase)
  - Verify: trigger eviction mid-export, assert full file or `TraceEvicted`, never partial
_No new tasks from Section 4 (Performance) — no findings this phase._

---

## Final Approval Gate (autoplan, 2026-09-13)

Four items required the human's judgment: 1 User Challenge (STOP #8) and 3 STOP-list Taste
Decisions (STOP #2, #4, #5) that no review voice was allowed to auto-decide. Presented to the
owner; resolved as follows.

| Item | What was asked | Decision | Rationale given |
|---|---|---|---|
| **USER CHALLENGE — STOP #8** | Keep the 9-10wk full v1, or switch to the 3-4wk diagnostics wedge, given the unresolved job-search timeline? | **Keep the full 9-10wk v1 as planned.** No change to milestone sequencing. | Owner's call — original direction stands per autoplan's rule that the user's stated direction is the default unless they change it. |
| **STOP #2 — UI port** | `:5317` (both DX voices' independent recommendation) or a different port? | **`:5317`.** | Matches both independent review voices; groups visually with OTLP's fixed 4317/4318; low collision risk with other local dev servers. |
| **STOP #5 — `--max-memory` default** | What value ships when the flag is omitted? (Always overridable via `--max-memory <n>` either way.) | **1 GiB**, provisional. | Matches the spec's own stated assumption and both independent DX voices; re-pinned after M1 measures real bytes/span (spec 02 acceptance #6). |
| **STOP #4 — dedupe semantics** | Last-write-wins or drop-on-duplicate for a duplicate `(trace_id, span_id)`? | **Last-write-wins.** | Matches how OTel SDKs commonly patch/retry spans (correcting end-time or adding attributes on resend); judged lower-risk than silently discarding a legitimate correction. |

**Spec files updated to reflect these decisions:** `01-m0-store-ingest.md` (§T4, §5 Retention,
STOP-items footer), `05-ui-surfaces.md` (D4 empty state), `07-cli-demo-docs.md` (CLI flag table,
STOP-items footer, startup print added), `04-ui-foundation.md` (STOP-items note), `02-m1-gate.md`
(bytes/span unblock table, acceptance #6), this file's STOP list (top of file), `docs/prd.md`
(Success Criteria placeholder annotated, not filled in — no date was given).

**Still open, unchanged by this gate:** STOP #1 (product name), STOP #3 (license), STOP #6 (lint
rule count, pending T1), STOP #7 (UI framework, pending M1). None of these were presented at this
gate — none rose to a User Challenge or a Taste Decision needing resolution now; each has its own
stated trigger to resolve (T0, before-first-commit; T1's measurement; M1's measurement) elsewhere
in this document.

**Status: APPROVED.** Proceed to implementation per the execution order above (T0 → T1/T3→T2 →
spec 01 → specs 03/04/06/07 in parallel → spec 05).
