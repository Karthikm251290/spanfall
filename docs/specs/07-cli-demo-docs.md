# Spec 07 — CLI surface, demo, release docs

**Tasks:** T16, T15, T14, D14, and **CEO T18 (orphan — needs a decision)**
**Owns:** `src/main.rs`, `src/cli.rs`, `src/demo.rs`, `examples/`, `docs/`, `CHANGELOG.md`
**Preconditions:** none for T14/T15/D14. T16 needs spec 01's listeners.

`src/main.rs` is owned by this spec **alone**. Other specs describe the subcommands they need; this
spec wires them. That is the one file every lane would otherwise touch.

---

## The CLI surface, assembled

Collected here because it is scattered across five specs and nowhere stated whole.

| Invocation | Behaviour | Specified in |
|---|---|---|
| `tracescope` (no args) | Run. Listen on 4317 + 4318, serve the UI, open nothing. | 01, 05 |
| `tracescope list` | List resident traces. **Required** — `export`'s `TraceNotFound` message says "try `list`". | this spec |
| `tracescope lint` | Lint the **newest** resident trace | 03 |
| `tracescope lint --all` | Lint every resident trace | 03 |
| `tracescope lint <file.otlp>` | Lint a capture via the Stage 2 load path | 03 |
| `tracescope export <id> [--force] [--redact]` | Single-file HTML export | 06 |
| `--demo` | Curated fake trace set | this spec |
| `--dump` | Prove M0: spans, flags, rejects, counters | 01 |
| `--max-memory <n>` | Retention cap, in bytes (default **1 GiB**, STOP #5 resolved 2026-09-13; always overridable) | 01 |
| `--ingest-timeout <t>` | How long a blocked send waits before returning a retryable 429 / `RESOURCE_EXHAUSTED` | 01 |

**Not in v1**, deferred in `TODOS.md`: `--open`, copy-trace-id / copy-as-curl, `--host` or any
non-localhost bind, CI `assert` mode, watch mode.

`--redact` is a flag on `export`, not a global.

> **STOP #1.** Every `tracescope` above is a placeholder. Derive the binary name from Cargo metadata
> (`env!("CARGO_BIN_NAME")` or clap's automatic name) so T0's resolution is a one-line change, not a
> repo-wide replace. **Help text and error messages must not hardcode it either.**

> **STOP #2 — resolved: `:5317`.** Decided 2026-09-13 at the `/autoplan` gate. Startup print (T16,
> plain `tracescope` invocation, no args):
> ```
> Listening on http://localhost:5317
> OTLP gRPC: 4317   OTLP HTTP: 4318
> ```
> Applies here, in D4's empty state, and in the README.

---

## T16 — port-conflict detection

**Effort:** ~3h human / ~30min CC.

When 4317 or 4318 is already bound, **name the process holding it** — pid and process name — not
`address already in use`.

Why it is in v1 rather than the delights bundle it started in: it is a **guaranteed** first-run
failure. Anyone who has ever run Jaeger, the OTel Collector, or another local viewer has something on
4317. A bind error at that moment reads as "this tool is broken," and it is the very first
interaction.

This is the same work as **CEO T17**. Two numbers, one task. Build it once.

---

## T15 — demo mode and the committed fixture

**Effort:** ~1d human / ~2h CC.

Two deliverables:

1. `--demo` — a curated fake trace set. Its stated value: adoption lever, README GIF source, and
   something for the linter to demonstrate. Near-free because M1 needs a synthetic generator anyway.
2. **`examples/checkout.otlp`** — one committed capture, so `tracescope lint examples/checkout.otlp`
   demonstrates the linter at **zero build cost** and with no running service.

### Two constraints that were dropped — do not reinstate them

- **Demo does not need to be sequenced after the final rule set.** It was going to be the linter's
  live fixture; under file linting the fixture is the committed `.otlp`, so demo is independent work
  and can run in any lane.
- **Demo does not need a synthetic resource attribute to exclude it from lint** (this was CEO T16,
  now dissolved). `lint` reads whatever is resident when you ask, and demo ships as a fixture rather
  than as a live source sharing lint state. **There is no shared lint state to exclude it from.**

The fixture should lint to **real findings on purpose** — it is the trace the rules were written
against, and it is what a reader runs first.

---

## T14 — re-cost the milestone table

**Effort:** ~2h human / ~20min CC. **The single task that unblocks honest scheduling.**

`docs/prd.md` §Milestone Plan states 9–10 weeks and then carries a table that was never updated: the row itself
admits "the table below still needs the expansions distributed across milestones."

Three things to fold in:

1. **The CEO expansions** — linter (~1–1.5 weeks), HTML export (~1 day), `--demo` (~half day). These
   added ~2.3 weeks and produced the 9–10 week figure, but were never distributed across milestone
   rows.
2. **D1–D14** — roughly **4–5 days of UI work that is not in the 9–10 week figure at all.** This is
   the number most likely to be wrong first.
3. **The slip rule must become meaningful again.** It currently says "if M1 has not cleared by end of
   week 3, cut the M3 panel" — against a table whose week numbers no longer reflect the scope.

Also fix, while in this file: `docs/prd.md` §Success Criteria → Lagging Indicators still carries `[PLACEHOLDER]` for the job-search
timeline (**STOP #8**), which the CEO review holds can invalidate the plan's shape outright. Do not
fill it in. Flag it.

---

## D14 — stale documentation lines

**Effort:** ~30min human / ~10min CC. **Partly done already.**

Already fixed in-review on 2026-09-13: `docs/prd.md` gained an **Interface surface (v1)** block and
Story 1's no-restart acceptance condition; the design doc's Lint tab mockup was corrected for the
scope contradiction.

Remaining:

| Stale text | Correct state |
|---|---|
| "Session-scoped unless stated" | No session scope exists. Scope is per-trace or all-resident. |
| "All thresholds configurable" | Hardcoded constants in v1. Config deferred to `TODOS.md`. |
| `/instrumentation` | Retired. The route is `/lint`, backed by `GET /api/lint`. |

Verify: grep finds no `/instrumentation`, and no "session-scoped" outside a strikethrough.

**Also in `TODOS.md`:** renumber the CEO half's task IDs to C1–C3 before T0. `docs/prd.md` now cites
T14, and there are two different T14s across the two series.

---

## CEO T18 — semver policy and CHANGELOG ⚠️ ORPHAN

**Effort:** ~1h human / ~15min CC. **Needs a keep-or-cut decision.**

**Status:** this task appears in the CEO review's task list and in the distribution diagram, and it
was **never carried into the engineering T0–T16 list.** Confirmed absent from
`tasks-eng-review-20260913-123124.jsonl`. It was not superseded and not deliberately cut — it fell
through the gap between two review passes.

**Why it was called load-bearing:** distribution is a single binary from a GitHub Release, and
**rollback is "the user pins an older tag."** That makes semver and a changelog part of the recovery
path, not documentation hygiene. If a release breaks someone's ingest, the changelog is how they find
out which tag to pin.

Deliverables if kept: a stated semver policy (what a breaking change means for a local dev tool —
notably a persistence format change, since Stage 2's container is versioned), and `CHANGELOG.md`
maintained per release.

**Recommendation: keep it.** It is ~1h and it is the documented rollback mechanism. But it is a real
scope item that nobody has approved, so surface it rather than silently building it.

---

## Acceptance

Cite the test plan: the port-already-bound case, the `--demo` case, and the
`lint examples/checkout.otlp` parity case.

| # | Check |
|---|---|
| 1 | Bind 4317 with another process, start tracescope: the error names the **holding process**, not "address already in use". |
| 2 | Same for 4318. |
| 3 | `tracescope list` exists and its output makes `export`'s "try `list`" message true. |
| 4 | `--demo` produces a trace set that lints to findings on purpose. |
| 5 | `lint examples/checkout.otlp` works with **no running service** and matches linting the same capture live. |
| 6 | Grep the binary's help text and all error strings: the product name appears from Cargo metadata only, never hardcoded. |
| 7 | The re-costed milestone table distributes the CEO expansions **and** D1–D14, and the slip rule references real week numbers. |
| 8 | `grep -r /instrumentation docs/` returns nothing. |
| 9 | A decision was recorded on CEO T18 — kept or cut, either is fine, silence is not. |

## Out of scope

`--open`, copy-trace-id, copy-as-curl (`TODOS.md`, P3 delights). Any non-localhost bind — the first
`--host` flag or tunnel is the documented trigger for responsive live-UI work, and it is not v1. CI
assert mode. Filling in the job-search timeline placeholder: that is the owner's, and it is STOP #8.

## STOP items live here

**#1** (name, in every help string, still open), **#2** (UI port, resolved to `:5317`, printed on
startup), **#5** (`--max-memory` default, resolved to 1 GiB, a flag this spec declares), **#8**
(scope question resolved 2026-09-13 — keep the full v1 build, do not switch to the diagnostics
wedge; the underlying job-search-timeline placeholder in `docs/prd.md` remains unfilled since no
date was given, but it no longer gates scope).
