# Spec 04 — UI foundation (framework-agnostic)

**Tasks:** T9, D8, D9, D12 (logic half), D3 (state half)
**Owns:** `ui/src/datasource.ts`, `ui/src/tokens.css`, `ui/src/keys.ts`, `ui/src/nav.ts`,
`src/assets/fonts/`
**Preconditions:** spec 01 §6 (the read API contract).
**Not blocked by STOP #7.** Everything here is plain TypeScript modules and CSS — no components, no
framework assumption. That is why it is split from spec 05.

---

## T9 — the `DataSource` seam

**Effort:** ~3h human / ~30min CC. **Lands *with* M2's waterfall, not after it.**

Four methods, one interface, two implementations:

```ts
getTrace(id)                     // columnar JSON + string table, no attributes
getSpanAttributes(id, spanIdx)   // one span's attributes
getFilterMatches(id, q)          // span indices only
subscribe(onChange)              // invalidation notifications
```

| Implementation | Backing |
|---|---|
| `HttpDataSource` | The live server, spec 01 §6 |
| `InlineDataSource` | Data embedded in an exported HTML file, spec 06 |

**No direct `fetch` anywhere in the UI.** That is the constraint that makes the export work.

### Why a seam and not a fetch stub

The viewer was originally specified against three server endpoints. An export has **no server.** A
fetch stub in an exported file fails at runtime, silently, in someone else's browser — the worst
possible place. The seam makes an unsupported call an **explicit** unsupported result instead.

**The export payload is a different shape, not a superset.** `InlineDataSource` has all attributes
inlined (there is no server to fetch them from), while `HttpDataSource` deliberately never receives
them in the trace payload. Do not write one payload type with optional fields and hope; they are
two shapes behind one interface.

`getFilterMatches` is **unsupported** in `InlineDataSource` — exports are view-only, and a second
filter implementation in JS is rejected by name (spec 01 §6). Return an explicit unsupported
result; do not throw a generic error and do not return an empty match set, which would read as "no
results."

---

## D8 — the design token block

**Effort:** ~2h human / ~20min CC. `ui/src/tokens.css`.

One block, read by **both** the live viewer and the exported file, since they share one
`rust-embed` asset set. Two surfaces sharing one bundle is precisely why the tokens must exist:
without them the two drift.

Light palette, verbatim:

```css
:root {
  --font-ui:   "IBM Plex Sans", system-ui, sans-serif;
  --font-mono: "IBM Plex Mono", ui-monospace, monospace;  /* all ids, counts, durations */

  --bg: #fbfbfa; --panel: #ffffff; --line: #e3e3e0;
  --ink: #17171a; --dim: #6b6b73; --faint: #9a9aa2;
  --sev-error: #b4342a; --sev-warn: #9a6400; --sev-info: #6b6b73;
  --link: #2b58c4;

  --sp-1: 4px; --sp-2: 8px; --sp-3: 12px; --sp-4: 16px; --sp-5: 24px; --sp-6: 40px;
  --radius: 4px;                    /* one value, everywhere */
}
@media (prefers-color-scheme: dark) { :root { /* dark values */ } }
```

### The dark palette values do not exist yet — derive them

The design doc writes the dark block as a comment. **Only the ten colour tokens are redefined in
dark**: `--bg`, `--panel`, `--line`, `--ink`, `--dim`, `--faint`, `--sev-error`, `--sev-warn`,
`--sev-info`, `--link`. Fonts, the spacing scale and the radius are palette-independent and must
**not** be repeated in the dark block.

Derive the values to satisfy the contrast rule below. This is not a STOP item because it has a
measurable acceptance criterion rather than a taste judgement — but it is *derivation work*, not
transcription.

### Rules that go with the tokens

- **Every severity colour must measure ≥4.5:1 against `--bg`, in both palettes.** The amber
  (`--sev-warn`) is the one that normally fails on dark. **Adjust the token, never the rule.**
- **Tabular numerals on every duration and count column**, or the waterfall's numbers jitter as
  rows scroll.
- **Theme the browser surfaces from these tokens**: selection, caret, scrollbars, focus ring,
  underline offset. Leaving them at browser defaults is the cheapest tell that nothing was designed.
- **More space above a heading than below it.** One radius value. **No shadows in v1** — there is no
  depth in this UI, so a shadow would be decoration.
- Light/dark follows `prefers-color-scheme` with **no toggle and no stored state.** This was pulled
  out of a P3 bundle and into v1 because it determines the *shape* of the token block; deferring it
  means writing one palette and rewriting it later.

---

## D9 — embed the two faces

**Effort:** ~2h human / ~20min CC. `src/assets/fonts/`, `ui/src/tokens.css`.

IBM Plex Sans (UI) and IBM Plex Mono (**every id, count and duration**). Latin subset, woff2,
embedded in the binary and inlined in exports. Budget ~40–60KB total.

Why it is not optional: shipping `system-ui` as the primary face is a documented "gave up on
typography" tell, and it was one of two blacklist items this plan actually triggered.

**The acceptance test is offline.** An exported file opened from `file://` with no network must
render both faces and make **zero network requests** — check the Network panel, not just the
appearance, since a fallback face can look close enough to pass a glance.

---

## D12 — keyboard model, as a module

**Effort:** ~4h human / ~40min CC (the logic half here; wiring to rows is spec 05).
`ui/src/keys.ts`.

| Key | Action |
|---|---|
| ↑ / ↓ | Move the focused row; the virtualized list keeps it **mounted and in view** |
| ← / → | Collapse / expand the focused subtree |
| Enter | Open span detail for the focused row |
| `/` | Focus the filter box |
| Escape | Clear the filter, **then** close the detail panel |

Two details that are easy to get wrong and are the whole point of the task:

1. **The focused row must survive list recycling.** Virtualization unmounts off-screen rows; a
   focused row that gets recycled loses focus and the keyboard model silently dies mid-scroll. Track
   focus by span index, not by DOM node.
2. **Escape is two-stage**, in that order. Not one handler that closes everything.

Semantics (applied to rows in spec 05, specified here so both specs agree): the waterfall is
`role="tree"` with `aria-level` and `aria-expanded` per row, and `aria-rowcount` for the
**virtualized total** — not the mounted count. Visible focus ring on every interactive element,
drawn from `--link`. Touch targets ≥44px in the exported file's narrow layout.

Lint findings are a list of headings, so a screen reader hears **tier, then finding, then count.**

---

## D3 (state half) — the navigation model

**Effort:** part of D3's ~3h. `ui/src/nav.ts`.

Three **peer** tabs: `Traces`, `Lint`, `Receiver`. The trace list is the landing surface. A trace
opens as a **layer over** the list, with an explicit back.

The requirement that makes this more than cosmetic: **clicking a lint sample lands on that span, and
back returns to the Lint tab — not to the trace list.** The return path must be part of the state
model, not inferred from history. Keep this as pure state here; spec 05 renders it.

---

## Acceptance

Cite the test plan: the keyboard/screen-reader cases, the fonts-offline case, the severity-contrast
case, the greyscale check, and the back-from-lint-sample case.

| # | Check |
|---|---|
| 1 | Grep `ui/src/` for `fetch(` — zero hits outside `datasource.ts`. |
| 2 | `InlineDataSource.getFilterMatches` returns an explicit unsupported result, not empty and not a throw. |
| 3 | Every severity colour measures ≥4.5:1 against `--bg`, **both palettes**, numbers recorded. |
| 4 | The dark block redefines exactly the ten colour tokens and nothing else. |
| 5 | Exported file from `file://`, network disabled: both faces render, **zero network requests** in the Network panel. |
| 6 | Arrow-key walk across 40k rows: focused row stays in view and stays focused through recycling. |
| 7 | Escape clears the filter first, closes the panel second. |
| 8 | `aria-rowcount` reports the virtualized total, not the mounted row count. |
| 9 | Greyscale screenshot of lint findings: tier still readable from the headings alone. |
| 10 | Lint sample → span → back lands on the Lint tab. |

## Out of scope

Any component. A light/dark **toggle** — system preference only. A full `DESIGN.md` — these ~15
tokens are the whole system for v1. **Vim keybindings and a `?` overlay** (`TODOS.md`, P3; trigger
is the first time someone presses `j` and nothing happens). A responsive **live** UI — desktop-only,
stated; the export carries the narrow layout instead (spec 06). Animation: no entrance motion, no
transitions beyond the row highlight on new spans.

## STOP items

None block this spec. **#2** (the UI port) surfaces in the empty state that spec 05 renders, and
**#7** (framework) gates spec 05 — neither gates anything here.
