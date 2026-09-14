# Spec 06 — single-file HTML export

**Tasks:** T10, T11, T12, T13, D10, D11
**Owns:** `src/export/`, `ui/src/export.css`
**Preconditions:** spec 04 complete (`InlineDataSource`, tokens, fonts). Spec 05 is *not* required
to start, but the export renders the same components, so it cannot be finished before they exist.
**Registers CLI subcommands** (`export`, `--redact`, `--force`) described here and wired by spec 07.

---

## Why this spec has the highest stakes in the project

The exported file is the **only artifact that leaves the developer's machine.** It is opened by
other people, on other devices, possibly months later, with the tracescope process long dead.

Three consequences that drive every decision below:

1. **It cannot be patched after it is sent.** There is no server, no update path, no telemetry.
2. **A stored-XSS bug here executes in a third party's browser**, from a file they were told was a
   debugging artifact. This was the review's only CRITICAL finding.
3. **Its success case is credential exfiltration.** A working export of a real production trace may
   carry `Authorization` headers, tokens in URLs, and PII — and it works perfectly while doing it.

This is also why the export, not the live UI, is the surface that gets responsive layout and
embedded fonts. It is the one that travels.

---

## T10 — the export itself

**Effort:** ~1.5d.

```
tracescope export <trace-id> [--force] [--redact]
```

| Property | Decision |
|---|---|
| Span cap | **~5,000 spans.** Over that, refuse. `--force` is the escape hatch. |
| Attributes | **All inlined.** There is no server to fetch them from. |
| Interaction | **View-only.** Waterfall + span detail. |
| Filter box | **None.** Not a disabled one — none. |
| Data source | `InlineDataSource` (spec 04) |
| Write | **Temp file, then atomic rename.** Never a partial file on disk. |
| Filename | `{service}-{trace-id-short}-{UTC-timestamp}.html`, lands in the current working directory. Example: `checkout-4bf92f35-20260913T150000Z.html`. Printed to stdout on success (see T13's stream split), so the redirect case (acceptance #6) has something to redirect. **`{service}` comes from `service.name` — a resource attribute the instrumented app sets, i.e. attacker/misconfiguration-controlled data, unlike `{trace-id-short}` which is a hex slice of a fixed-width binary trace ID and cannot contain path separators.** Sanitize `{service}` before it reaches the filesystem: keep only `[a-zA-Z0-9._-]`, replace every other byte (including `/`, `..`, null) with `_`, and cap the sanitized result at 64 bytes. This is a filesystem-safety control, distinct from T11's HTML-escaping of the same string when it is later *displayed* inside the exported page — both are required, neither substitutes for the other. **Multi-service trace:** use the root span's (the span with no parent) `service.name`. |

### Why the cap exists, and why the filter box does not

Uncapped export reconstructs the **fully-hydrated-trace design that `docs/prd.md` rejects by name** —
as the flagship shared artifact, and with the filter box removed the reversal would have been
invisible. The cap is what keeps that rejection closed. (The owner initially chose uncapped; the
outside-voice pass capped it. The cap is the settled position.)

A filter box would be a **second filter implementation in JS**, whose semantics would have to track
the Rust one used for cross-trace search. Also rejected by name. The provenance bar states that
filtering is unavailable — a dead control would say it twice.

With `--force`, a post-serialization **size warning** fires near **GitHub's 25 MB attachment
limit**, because the realistic destination for this file is a pull request or an issue.

### Error handling

| Error | Action | User sees |
|---|---|---|
| `TraceNotFound` | exit 1 | ``no trace `<id>`; try `list` `` |
| `TraceEvicted` | exit 1 | `evicted (retention cap)` |
| `io::Error` | exit 1, **no partial file** | the OS error verbatim |
| `ExportTooLarge` | refuse | the size **and the `--force` flag by name** |
| secrets present | **export anyway**, warn | the key list with counts, then the file path |

The `TraceNotFound` message references a `list` subcommand — spec 07 must provide it, or the message
points at nothing.

---

## T11 — XSS, and the exact escape

**Effort:** ~2h. **This is the CRITICAL finding's test task.**

### The mechanism

Span data goes into ``<script type="application/json">``, is read with `JSON.parse`, and is rendered
**`textContent` only — never `innerHTML`.**

Inside that JSON, the `<` character is serialized as the six-character JSON escape
`\u003c` (backslash, u, 0, 0, 3, c).

### The distinction that matters, stated plainly

`\u003c` is a **JSON escape, not HTML-escaping.** Conflating the two lands you in a
double-escaping bug. Concretely:

- The JSON escape stops a ``</script>`` sequence inside a string value from breaking out of the
  script element. That is its entire job.
- **HTML-escaping the value instead would mangle legitimate content.** A span named
  `Handler<Request>` would render as `Handler&lt;Request&gt;`, visibly wrong, in a tool whose users
  are developers reading generic type names all day.

**`textContent`-only rendering is the actual control.** The escape only prevents the
``</script>`` breakout. Both are required; neither substitutes for the other.

### The full hostile surface — one test per field

A pass on span name alone is **not coverage.** Each of these is independently attacker-controlled:

span names · attribute **keys** · attribute values · resource attributes · status messages · event
names · event attributes · link attributes · the document `<title>` · **the output filename**

Two canonical cases:

| Input | Required result |
|---|---|
| Span name ``</script><img src=x onerror=alert(1)>`` | Renders as **visible text**, never executes — live viewer **and** exported file |
| Span name `Handler<Request>` | **Angle brackets intact.** This is the test that proves the JSON escape was used and not HTML-escaping. |

---

## T12 — CSP inside the exported file

**Effort:** ~30min.

A `<meta>` CSP of `default-src 'none'` inside the exported file, as **defense in depth.**

**Additive**, not a replacement for T11's escaping and `textContent` discipline. It was adopted as an
*addition to* the primary fix, explicitly not instead of it.

The file must remain **fully functional** with that CSP present — which it can be, since it makes no
network requests, loads no external scripts, and inlines its fonts.

---

## T13 — secret scan

**Effort:** ~3h.

**Committed pattern list**, scanning attribute keys and value shapes:

| Kind | Patterns |
|---|---|
| Keys | `authorization`, `cookie`, `token`, `secret`, `password`, `api_key`, `credentials` |
| Value shapes | `Bearer `, `sk-`, `eyJ` |

Behaviour: **print what was found with counts, then export anyway.** Not a block — a developer
exporting a trace they understand should not be prevented.

### The stream split is load-bearing

**The warning goes to stderr. The written path goes to stdout.**

So that `tracescope export <id> > /tmp/path.txt`, or any pipeline consuming the path, **still shows
the warning to the human.** A warning on stdout would be captured into the file and never read.

`--redact` is **opt-in**. With it, matched values read `[redacted]`. The file must open correctly
both ways.

Residual risk, accepted and recorded: a developer ignores the warning. The mitigation is that the
warning names the keys and counts, so it is specific rather than generic.

---

## D10 — the provenance bar

**Effort:** ~2h.

**The problem it solves:** an exported file looks *identical* to the live tool, but it is frozen and
cannot filter. Someone handed the file will try to use it as the live tool and conclude the tool is
broken.

A **persistent** bar stating, all of it:

- **SNAPSHOT**
- exported from (which machine/service)
- **frozen**
- **filtering is unavailable**
- the operation name
- the span count
- the **UTC** time
- **`N of M spans (capped)`** when the trace was truncated at the ~5k cap

The capped case is the one most likely to mislead: a reader who does not know spans are missing will
draw conclusions from a partial trace.

---

## D11 — narrow layout, exports only

**Effort:** ~3h. `ui/src/export.css`.

Below **~700px**, for the **exported viewer only.** The live UI is declared desktop-only.

- **No horizontal scroll on the document.** The waterfall keeps its own `overflow-x` scroll
  container; the page itself must never scroll sideways.
- Tap targets **≥44px**.
- Span detail opens as a **full-width sheet**, not a side panel.

The reasoning for spending responsive effort here and nowhere else: this is the surface that gets
opened on a phone, by someone who was sent a link in a chat, and it cannot be fixed afterwards.

---

## Acceptance

Cite the test plan: the full XSS surface case, the `Handler<Request>` case, the CSP case, the
kill-the-process-and-open case, `--redact` on and off, the 5,001-span case, the provenance-bar case,
the 375px case, the fonts-offline case, the theme-follows-the-reader case, and the disk-full case.

Spec-specific additions:

| # | Check |
|---|---|
| 1 | Export, **kill the tracescope process**, open the file: waterfall renders, span details open, no filter box, **zero network requests**. |
| 2 | One hostile-input test **per field** in the list above, both live viewer and exported file. |
| 3 | `Handler<Request>` keeps its angle brackets. |
| 4 | The file is fully functional with the `default-src 'none'` CSP present. |
| 5 | Export at **5,001 spans** is refused, and the message names `--force`. With `--force` it completes and the 25 MB size warning fires. |
| 6 | `tracescope export <id> > file`: the path lands in the file, the secret warning still appears on the terminal. |
| 7 | `--redact` on and off both produce openable files; redacted values read `[redacted]`. |
| 8 | Provenance bar states SNAPSHOT, source, frozen, filtering-unavailable, operation, span count, UTC time — and `N of M spans (capped)` when truncated. |
| 9 | At 375px: no horizontal page scroll, ≥44px targets, span detail as a full-width sheet. |
| 10 | A file exported from a **dark-mode** machine renders **light** for a recipient whose system is light. |
| 11 | Disk full mid-write: **no partial file left behind**, OS error shown. |
| 12 | **(eng review, 2026-09-13)** Trigger eviction of the trace being exported mid-export: the export either completes with the full trace it started reading, or fails with `TraceEvicted` — never a partial/truncated file. This should already hold structurally (spec 01 §4: readers copy out everything they need before releasing the read lock, so an evicting writer blocks until export's copy finishes), but is not yet an asserted test. |

## Out of scope

**Subtree export** (`TODOS.md`; the trigger is now the ~5k cap being hit, not a size warning). A
filter box, disabled or otherwise. **Parquet export** — v1.1, behind a feature flag, where
`arrow-rs` earns its dependency. Any server dependency in the exported file. Editing or annotating
an exported trace.

## STOP items

None block this spec.
