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
