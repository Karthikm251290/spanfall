# TODOS

## Architecture

### Watch item: cross-trace attribute search would break the trace-partitioned model

**What:** The v1 store is `HashMap<TraceId, Arc<TraceSlab>>` — fast for "open trace X" (O(1)) and for filtering within one open trace, but there is no index across traces. A feature like "find every trace containing a span where `http.status_code = 500`" would require scanning every slab.

**Why:** This is the one requirement change that would invalidate the v1 storage decision (`/plan-eng-review`, 2026-09-13). It is deliberately NOT in the PRD's v1 scope — Story 2 filters within an open trace, and the Success Criteria latency targets are all single-trace. Worth knowing the tripwire so it's a conscious decision if it comes up, rather than a slow slide into needing an engine.

**Context:** If cross-trace search becomes a real requirement, the options in order of cost: (1) maintain an inverted index from attribute key/value → trace_id on insert, which is cheap and probably enough for a local tool; (2) bring DataFusion in over persisted Parquet, which is already the Stage 2 plan for cross-trace/persisted queries anyway. Don't reach for the engine for the in-memory hot path — see the superseded decision in `~/.gstack` for why that was reversed.

**Effort:** M (if it happens)
**Priority:** P3
**Depends on:** A real user request for cross-trace search. Don't build speculatively.

### Name the project before the first commit

**What:** Decide the real name. The directory is `spanfall`, the PRD calls `tracescope` a placeholder.

**Why:** It is now genuinely blocking rather than cosmetic: it goes into the crate name, the binary name, the config env-var prefix, and the repo URL. Renaming after the first public commit costs a broken install path for anyone who found it early.

**Context:** PRD's "Before Finalizing" checklist already requires checking crates.io, GitHub, and npm for collisions. Do that check and pick, same sitting.

**Effort:** S
**Priority:** P0
**Depends on:** None.

## Product (deferred from CEO review 2026-09-13)

Full reasoning and the accepted-vs-deferred table live in
`~/.gstack/projects/spanfall/ceo-plans/2026-09-13-tracescope-v1.md`.

### Watch mode with diff in the edit-run loop

**What:** `tracescope watch` shows the delta from the previous run of the same operation:
"POST /checkout 340ms → 890ms. New: 12 spans under db.query (N+1 introduced). Slower:
payment.authorize +140ms."

**Why:** Turns a passive viewer into a regression detector inside the edit-run loop. Feels
like `git diff` for performance. Every incumbent is passive; this is the strongest version
of "the tool talks back."

**Context:** Deferred because it reframes the product enough that the PRD's problem
statement would need rewriting rather than amending. Revisit after the verification spike
resolves the positioning question.

**Effort:** L (human ~1 week / CC ~1 day)
**Priority:** P2
**Depends on:** Trace diffing (already Stage 2) plus operation-identity matching across runs.

### CI assert mode

**What:** `tracescope assert --max-spans 500 --no-orphans --no-n-plus-one` exits nonzero.

**Why:** Makes traces a test surface. "My CI catches N+1 queries from traces" is a
capability that does not exist in any tool today.

**Context:** This is the linter with an exit code, so it becomes cheap once the linter
lands in v1. Natural v1.1 headline feature.

**Effort:** S (human ~3 days / CC ~4h)
**Priority:** P2
**Depends on:** Instrumentation linter (accepted into v1 scope).

### Terminal-first tree output

**What:** Print the trace as a tree in the terminal, well enough that the browser is often
unnecessary.

**Why:** Unoccupied positioning — otel-tui is terminal-only, everyone else is browser-only.
It is also the surface where the linter's warnings land most naturally in the platonic-ideal
flow from the CEO review.

**Context:** Real cost is a second UI surface to maintain in perpetuity, with its own
layout and truncation rules. Wants a deliberate yes, not a casual add-on. Note the tension:
the CEO review's platonic ideal is terminal-first, but v1 ships the linter into the web
Receiver panel instead, so v1 does not yet deliver that felt experience.

**Effort:** S-M (human ~3 days / CC ~4h)
**Priority:** P2
**Depends on:** None technically; depends on a positioning decision.

### MCP server so agents can query traces

**What:** Expose the trace store over MCP so an AI agent debugging a service can query its
own telemetry.

**Why:** Agent loops are named in the PRD as a primary source of huge traces. The agents
producing those traces currently cannot read them. A trace store an agent can query is a
genuinely 2026-shaped idea.

**Context:** Speculative demand — no evidence gathered, unlike the diagnostics pain which
OTel's own survey documents. Cheap to build on top of the existing query layer if it ever
looks warranted.

**Effort:** S (human ~3 days / CC ~4h)
**Priority:** P3
**Depends on:** Query layer (v1).

### Small delights bundle

**What:** ~~Port-conflict detection that names the process holding 4317~~ (pulled into v1 by
the CEO deep review, 2026-09-13 — `AddrInUse` was the one unrescued error in the registry);
`--open` to launch the browser at the newest trace; copy-trace-id and copy-as-curl from the
span detail panel; system-preference dark mode.

**Why:** All polish upside, near-zero risk. Polish is part of what separates a repo people
star from one they close, and the incumbents' UIs are not a high bar.

**Context:** Bundled deliberately — each item is too small for its own decision. Port
conflict detection is the highest-value one, since "address already in use" is a real
first-run failure for a tool that binds two well-known ports.

**Amended 2026-09-13** (design review): ~~system-preference dark mode~~ pulled into v1 — both
palettes live in the design-token block, so deferring it would have meant writing the tokens
for one mode and rewriting them later. Two of four items now remain.

**Effort:** S (human ~half day / CC ~1h) — was ~1 day before dark mode moved into v1
**Priority:** P3
**Depends on:** None.

### Subtree export (`export <trace-id> --from <span-id>`)

**What:** Export one branch of a trace as a shareable HTML file instead of the whole thing.

**Why:** Nobody sharing a bug report needs all 40k spans — they need the slow branch. It is
also the answer to the export size problem: v1 exports the full trace at any size and only
*warns* when the file approaches GitHub's 25 MB attachment limit, so a large trace is
shareable-in-principle but awkward in practice.

**Context:** Considered and declined for v1 (CEO deep review, Finding 8 — the owner chose
uncapped full export over a capped export plus this command). Trigger to build: the first time
the size warning actually fires on a real trace someone wanted to share.

**Effort:** S (human ~3h / CC ~30m)
**Priority:** P2
**Depends on:** Export (v1).

### `--json` lint output

**What:** A `--json` flag on the lint command that emits the `lint::Finding` list as JSON
instead of formatted text.

**Why:** Once the structured `lint::Finding` type lands (eng review T7, 2026-09-13), the
terminal and the web API are both thin formatters over one type — a third formatter is a
serde derive and a match arm. It makes the linter scriptable: pipe to jq, diff between runs,
post to a PR comment, without anyone regex-parsing terminal text.

**Context:** CI assert mode (P2 above) is the natural consumer, and **assert mode is roughly
this plus an exit code** — that relationship is the useful part to remember. Deferred out of
v1 rather than built because an output format shipped before it has a consumer tends to be
the wrong shape and then has to stay stable.

**Effort:** XS (human ~1h / CC ~10m) once T7 exists
**Priority:** P3
**Depends on:** T7 (`lint::Finding` type). Blocked on nothing else.

### Lint threshold config file

**What:** Let users override the six hardcoded rule thresholds (span-count ceiling, orphan
ratio, cardinality limit, etc.) from a config file.

**Why:** Thresholds that ship as constants will be wrong for someone. But building config
before a single user has complained is speculative generality — the first complaint tells you
which threshold is wrong and by how much, which is information a config file cannot give you.

**Context:** Deferred by the CEO deep review (2026-09-13, Section 10). Trigger to build: the
first real user reporting a rule as too noisy or too quiet on their app. CI assert mode
(P2 above) probably wants this first, since a CI threshold has to be per-project.

**Effort:** S (human ~1 day / CC ~2h)
**Priority:** P3
**Depends on:** A real complaint. Do not build speculatively.

## Design (deferred from design review 2026-09-13)

Full findings and the approved directions live in `docs/designs/tracescope-v1.md`, section
"Design Review". Wireframes: `~/.gstack/projects/spanfall/designs/lint-tab-20260913/`.

### Renumber the task IDs in `docs/designs/tracescope-v1.md` (do before T0)

**What:** the doc has two independent T-series for different work — the CEO half uses T16/T17/T18,
the eng half uses T0–T16. There are two different T14s. Design tasks were numbered D1–D14
specifically to avoid making it worse.

**Why:** `docs/prd.md`'s new "Interface surface (v1)" block now cites T14 as the task that re-costs
the milestone table, so the collision is referenced from the architecture of record. It is the
first thing that confuses whoever picks up T14.

**Context:** noted during the eng review, deliberately not fixed mid-review. Mechanical: give the
CEO series its own prefix (C1–C3) and leave T0–T16 alone. Trigger: before T0, since T0 is the
first task anyone actually reads the list to find.

**Effort:** XS (human ~20min / CC ~5min)

### Vim keybindings and a `?` shortcut overlay

**What:** `j`/`k`/`h`/`l` aliases for the approved arrow-key row navigation, `g`/`G` for first
and last row, and a `?` overlay listing every binding.

**Why:** the audience is terminal-resident developers. Arrow keys work, but `j`/`k` is what that
audience reaches for, and a discoverable list is how anyone learns the bindings exist at all.

**Context:** option 10C in the design review; 10A (arrows + tree semantics) was chosen instead.
Cheap once 10A exists — the handlers are already written, this is aliases plus one overlay. Two
things to get right: `?` must not fire while the filter box has focus, and the overlay is a
surface that has to stay consistent with the bindings forever. Trigger to build: the first time
someone presses `j` and nothing happens.

**Effort:** XS (human ~2h / CC ~20min)
**Priority:** P3
**Depends on:** arrow-key row navigation shipping first.

### Responsive layouts for the live UI

**What:** narrow-viewport layouts for the live viewer too — trace list, Lint tab, Receiver
panel, waterfall — not just the exported file.

**Why:** the live UI binds to `127.0.0.1`, so today this work has no audience. That changes the
moment anything serves it beyond localhost: a `--host` flag, an SSH tunnel, a devcontainer, or
the Stage 3 cloud mode already sketched in the PRD.

**Context:** option 9B in the design review; 9A shipped the narrow layout for the *export* only,
because the export is the surface that travels and cannot be patched once sent. The export's
breakpoints do most of the design thinking, so this is largely applying them to three more
surfaces. Trigger to build: the first flag or tunnel that serves this UI to a device other than
the host machine.

**Effort:** M (human ~1 day / CC ~2h)
**Priority:** P3
**Depends on:** the export narrow layout landing first, so there is one set of breakpoints.

### A real DESIGN.md

**What:** a full design system — palette with intent, type scale, spacing, component
vocabulary, voice — rather than the ~15 CSS variables now written into the plan.

**Why:** the tokens settle v1, but they are a token list, not a design language. A second visual
surface needs an answer to "what does this product look like" that lives outside one plan file.

**Context:** option 8B in the design review; 8A (the token block) was chosen because most of a
DESIGN.md would restate what fifteen variables already say, and a design system with one
consumer is speculative generality. Best seeded from a built screen rather than written cold —
`design extract --image` can generate it from an approved mockup. Trigger to build: a second
visual surface, most likely the launch landing page if the spike clears.

**Effort:** S (human ~2h / CC ~30min)
**Priority:** P3
**Depends on:** nothing. Better *after* a real screen exists than before.

## Completed

### ~~Persistence schema versioning~~ — closed 2026-09-13, resolved by design

Was: define how a v1.0 Parquet file is read by a later version that added fields.

Resolved rather than scheduled. Persistence is now length-delimited OTLP protobuf behind a magic + format-version header, not Parquet. Protobuf gives forward and backward compatibility as a property of the format, so v1.1 adding logs does not break v1.0 files and no migration framework is needed. Parquet demoted to a v1.1 export feature, where a file-format break would be inconvenient rather than data-losing.

### ~~DataFusion prepared-plan fallback~~ — closed 2026-09-13, WONTDO

Was: a plan-caching mitigation if DataFusion's per-query planning threatened the 100ms filter budget.

Closed because it was a mitigation for a problem created by a dependency that should not be taken. v1 has no embedded query engine — the hot path is a hash lookup plus a sub-millisecond scan over ≤40k rows. See "Decided Against" in `docs/prd.md`.
