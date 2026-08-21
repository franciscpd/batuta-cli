# UI/UX Contract: UX Foundation

Terminal-interface contract for UX Foundation. The raw daemon transcript and
workspace catalog remain authoritative; this document defines presentation,
interaction, hierarchy, and responsive behavior.

## Experience Principles

1. **Conversation first** — user intent and assistant response dominate.
2. **Quiet until useful** — tools, system data, telemetry, commands, JSON, and
   paths begin summarized and expand in place.
3. **Evidence remains reachable** — grouping and summaries never destroy or
   reorder raw data; debug mode exposes it.
4. **State is redundant** — color reinforces a label/glyph/border, never
   replaces it.
5. **No surprise writes** — workspace registration requires a distinct review
   and confirmation step.
6. **Terminal-native** — default text inherits terminal colors, layouts wrap,
   and all required meaning survives 16 colors and `NO_COLOR`.

## Semantic Tokens

| Token | Meaning | 16-color dark | 16-color light | Non-color signal |
| --- | --- | --- | --- | --- |
| `text` | normal conversation/content | terminal default | terminal default | ordinary text |
| `muted` | secondary metadata | dark gray/dim | gray/dim | indentation/context label |
| `active` | focus/current/live | cyan + blue accent | blue + dark cyan accent | `>`/focus border/reverse |
| `success` | completed/succeeded | green | dark/standard green | `✓` and `completed`/`succeeded` |
| `waiting` | thinking/tool running/attention | yellow | dark yellow | `…`, `!`, and state label |
| `error` | failed/non-destructive error | red | dark/standard red | `×` and `failed`/`error` |
| `destructive` | action with destructive consequence | bright red + bold | red + bold | explicit destructive verb + confirmation |
| `system` | daemon/system/meta event | magenta, dim | dark magenta | `system` label and indentation |

Normal surfaces do not set a background color. Selection uses reverse video
because it inherits terminal colors. No token uses `Color::Rgb` or an indexed
color above 15. In `NO_COLOR`, foreground/background attributes are absent;
bold, dim, reverse, borders, glyphs, labels, and spacing remain.

## Transcript Information Hierarchy

Default order and weight:

1. User message — full body, explicit `user` label.
2. Assistant message — full body, agent label.
3. Current activity — one compact state line.
4. Attention/error — human statement and action visible; technical detail
   collapsed.
5. Tool summary — name, state, safe concise summary.
6. System/telemetry — discrete or grouped, muted, collapsed.

User and assistant prose is never ellipsized or replaced by a count. Commands,
JSON, absolute paths, tool inputs/outputs, and diagnostic payloads are not
printed inline in collapsed mode. A safe summary may use a short scalar or
basename, but must not leak a full payload merely because it is available.

## Transcript Rows

### Conversation and activity

```text
▸ user
  Add retry behavior without cancelling the session.

▸ batuta
  I’ll inspect the reconnect path and preserve the quit invariant.

… thinking
```

Activity states:

| State | Marker | Required label | Semantic token |
| --- | --- | --- | --- |
| thinking | `…` | `thinking` | waiting |
| tool | `▶` | `tool · <running state>` | waiting |
| waiting | `!` | `waiting · <reason>` | waiting |
| completed | `✓` | `completed` | success |
| failed | `×` | `failed` | error |

Only one turn-level activity state is current. Historical tool rows retain
their individual states.

### Collapsed and expanded tool

```text
▶ read_file · completed   workspace.rs
```

After `Enter`:

```text
▼ read_file · completed
  path
    /work/batuta-cli/crates/batuta/src/workspace.rs
  output
    <wrapped content>
```

The summary does not use the full absolute path by default. Expanded content
wraps and can extend vertically; it never triggers horizontal scrolling.

### Human-first error

```text
× workspace could not be added
  The daemon rejected the directory. Press Enter for technical details.
```

Expanded:

```text
▼ workspace could not be added
  The daemon rejected the directory.
  code       workspace_invalid
  diagnostic root_dir must be canonical
  raw        { ... }
```

The cause/action precedes codes, diagnostics, and raw payloads in every theme.

### Condensed events

```text
▶ 6 tool updates · completed   Enter expand
─ 9 telemetry events           Enter expand
```

Expanded groups show every member with its original state and source order.
Errors, attention, permission, clarification, user, and assistant boundaries
always break a group. Counts refer to raw source members, not rendered lines.

### Global debug mode

```text
DEBUG · raw transcript presentation
entry seq=144 start=139 role=assistant
part[0] type=dynamic-tool
{ ...full serialized source payload... }
```

Debug mode bypasses normal summaries and groups across the transcript. Raw
payloads may occupy multiple pages but are not normalized, truncated, or
discarded. The header visibly says `DEBUG`; color is not required to notice it.

## Wrapping

- Measure width by Unicode display columns, not bytes or scalar count.
- Preserve the semantic indentation of continuation lines.
- Prefer whitespace breaks; hard-break a token only when it exceeds available
  width.
- Wrap markdown prose and expanded payloads; never horizontally scroll the
  conversational transcript.
- Recompute presentation cache on width, theme, color, debug, expansion, or
  source-sequence changes.

## Follow-Tail

At the live tail, no extra banner is necessary. Moving upward suspends follow:

```text
──────────────── 12 new updates · G return live ────────────────
```

The count measures new raw transcript updates received since follow was
suspended. The viewport stays anchored to a stable source entry/part even when
groups above or below it change. `G` selects the newest content, resumes
follow-tail, and clears the count atomically.

## Adaptive Layout

Panel relevance order is deterministic:

```text
pending attention > focused > active non-terminal work > populated > empty
```

Stable panel order breaks ties: Sessions, Deliver runs, Attention. Transcript
is not part of that competition; it is the primary surface whenever detail is
available.

### Narrow: 80–99 columns

Exactly one content panel is rendered at a time.

```text
┌ batuta · ws new-project · daemon ok ┐
├ Session: batuta · spec workspace ───┤
│ ▸ user                              │
│   Register this directory safely.   │
│                                     │
│ ▸ batuta                            │
│   I’ll ask before changing it.      │
│                                     │
│ … thinking                          │
├─────────────────────────────────────┤
│ 1 sessions 2 runs 3 attention 4 chat│
└─────────────────────────────────────┘
```

When a session is open, transcript/detail is the default content panel.
`1`–`4` and existing focus navigation make every panel reachable. No hidden
30-column rail remains allocated.

### Medium: 100–139 columns

Transcript plus one contextual rail. The rail shows the highest-relevance
panel; other contextual panels reduce to compact header/status rows.

```text
┌ Sessions (4) ───────┬ Session: batuta ─────────────────────────┐
│ > spec workspace    │ ▸ user                                  │
│   fix retry         │   Register this directory safely.       │
│                     │                                         │
├ Runs (0) ───────────┤ ▸ batuta                                │
├ Attention (0) ──────┤   I’ll ask before changing it.          │
└─────────────────────┴─────────────────────────────────────────┘
```

An empty compact panel consumes only its header/status row. If attention
becomes pending, Attention becomes the grown rail without replacing detail.

### Wide: 140+ columns

Transcript receives at least 60% of content width. Contextual panels share the
rest; empty panels remain compact vertically and the relevant panel grows.

### Too small: below 80x24

The existing request for a larger terminal remains. No controls or content are
partially rendered.

## Workspace Onboarding

### Unregistered directory

```text
┌ Workspace not registered ─────────────────────────────────────┐
│                                                               │
│ Name   new-project                                            │
│ Path   /work/new-project                                      │
│                                                               │
│ [a] add this directory                                        │
│ [w] choose an existing workspace                              │
│ [q] exit                                                      │
└───────────────────────────────────────────────────────────────┘
```

Name and canonical path are never replaced by an inferred friendly message.
The add action is available only when canonicalization succeeded.

### Confirmation

```text
┌ Add workspace? ───────────────────────────────────────────────┐
│ Name   new-project                                            │
│ Path   /work/new-project                                      │
│                                                               │
│ This registers the directory with the connected daemon.       │
│ Enter confirm                                      Esc cancel │
└───────────────────────────────────────────────────────────────┘
```

Confirmation is a separate state, not a preselected button on the first
screen. `Esc` returns to onboarding and performs no write.

### Progress, unsupported, and failure

During the single confirmed request, show `… adding workspace` and suppress a
second confirmation. Unsupported and failure return to stable surfaces with
the candidate still visible. Technical details use the same expandable
human-first error pattern as transcript errors.

## Accessibility and Input

- Keyboard-only; no mouse capture is introduced.
- Focus is visible with a glyph/border/reverse treatment in all modes.
- Destructive or mutating actions state their verb and require existing
  confirmation conventions.
- No decorative animation or spinner is required; activity glyphs may change
  only when underlying state changes.
- Help text names the actual keys and remains synchronized with keymap tests.

## Required Visual Matrix

Every core surface must be reviewed across:

| Dimension | Values |
| --- | --- |
| Theme | dark, light, `NO_COLOR` |
| Color capability | ANSI 16 colors |
| Width | narrow, medium, wide |
| Transcript content | empty, short, long, tool, error, attention |

The concrete automated/snapshot combinations and behavioral assertions are in
`_tests.md`.
