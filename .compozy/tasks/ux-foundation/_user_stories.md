# User Stories: UX Foundation

Canonical behavior catalog for the UX Foundation delivery. Companion to
`_spec.md`; `_uiux.md` defines the terminal surfaces, `_dx.md` defines public
operator/developer contracts, and `_tests.md` maps every observable behavior.

## Personas

- **Conversation Operator** — follows long agent sessions and needs the actual
  user/assistant exchange to remain readable while tools and telemetry stream.
- **Incident Investigator** — expands errors, tool payloads, and raw events to
  establish exactly what happened without losing source fidelity.
- **Terminal Operator** — uses dark, light, low-color, remote, or `NO_COLOR`
  terminals and needs equivalent state information in each.
- **Workspace Starter** — launches `batuta` from a repository that may or may
  not already be registered and needs a safe path into the correct workspace.

## Story Index

| ID | Area | Persona | Story |
| --- | --- | --- | --- |
| US-001 | Visual language | Terminal Operator | Read a sober semantic interface that inherits terminal text |
| US-002 | Accessibility | Terminal Operator | Understand every state without relying on color |
| US-003 | Transcript | Conversation Operator | Read complete user/assistant messages as the primary narrative |
| US-004 | Progressive detail | Incident Investigator | Expand tools, telemetry, system, and error details on demand |
| US-005 | Fidelity | Incident Investigator | Condense routine events without losing count, order, or raw data |
| US-006 | Live reading | Conversation Operator | Scroll safely while new transcript updates arrive |
| US-007 | Layout | Terminal Operator | Keep the transcript primary at narrow, medium, and wide sizes |
| US-008 | Resolution | Workspace Starter | Select workspace by deterministic explicit/cwd precedence |
| US-009 | Onboarding | Workspace Starter | Understand and confirm an unregistered directory before adding it |
| US-010 | Registration | Workspace Starter | Enter the new workspace after a successful daemon registration |
| US-011 | Recovery | Workspace Starter | Recover from error or unsupported registration without hidden mutation |

## Visual Language

### US-001: Read a sober semantic interface that inherits terminal text

**As a** Terminal Operator, **I want** normal content to inherit my terminal
colors while status accents use stable semantic tokens, **so that** batuta
looks professional and remains legible in my environment.

Acceptance criteria:

- AC-1: Normal user/assistant text uses the terminal's default foreground and
  background, not a hard-coded white, gray, or black.
- AC-2: Active uses cyan/blue, success green, waiting/attention amber/yellow,
  error/destructive red, and system a restrained purple/magenta mapping.
- AC-3: Every mapping uses only the terminal default plus the ANSI 16-color
  palette; no feature requires RGB or 256-color support.
- AC-4: Dark and light variants preserve semantic meaning while choosing
  modifiers/bright variants that remain readable against their background.

Edge cases:

- EC-1: `auto` reads the final numeric `COLORFGBG` component (`0`/`8` dark,
  `7`/`15` light); absent, invalid, or other values use dark deterministically.
- EC-2: A terminal ignores `DIM` → labels and glyphs still establish hierarchy.

### US-002: Understand every state without relying on color

**As a** Terminal Operator, **I want** labels and symbols to accompany color,
**so that** color blindness, `NO_COLOR`, or a limited terminal does not erase
meaning.

Acceptance criteria:

- AC-1: Active, success, waiting/attention, error/destructive, and system states
  each include text, a glyph, a border/focus treatment, or a combination.
- AC-2: With `NO_COLOR`, no foreground/background color attributes are emitted
  and the same states remain distinguishable.
- AC-3: `NO_COLOR` overrides `ui.color` and `ui.theme`; changing color mode does
  not change content, ordering, actions, or layout.

Edge cases:

- EC-1: Success and waiting glyphs render at width one in a basic terminal →
  their textual labels still disambiguate them.

## Transcript

### US-003: Read complete user/assistant messages as the primary narrative

**As a** Conversation Operator, **I want** user and assistant messages shown in
full and in order, **so that** operational noise does not break the dialogue.

Acceptance criteria:

- AC-1: User and assistant text is never summarized, grouped away, or clipped
  by the presentation layer.
- AC-2: Markdown and plain text wrap to the available display width, preserving
  indentation and readable continuation lines.
- AC-3: System entries are discrete and visually subordinate; tools and
  telemetry never receive the same default visual weight as conversational
  prose.
- AC-4: The active turn shows exactly one textual state among `thinking`,
  `tool`, `waiting`, `completed`, and `failed`, plus a non-color marker.

Edge cases:

- EC-1: A message contains a long unbroken token → it soft-wraps instead of
  overflowing or disappearing.
- EC-2: Empty transcript → render a compact, helpful empty state without fake
  activity or telemetry.
- EC-3: Streaming text changes length while wrapping → selection remains tied
  to the same source entry.

### US-004: Expand operational detail on demand

**As an** Incident Investigator, **I want** concise tools and human-first errors
that expand with `Enter`, **so that** I can move from overview to evidence
without leaving the transcript.

Acceptance criteria:

- AC-1: A collapsed tool row shows tool name, state, and a concise safe summary;
  command text, JSON, paths, input, and output remain collapsed.
- AC-2: A collapsed error shows a human-readable cause/action before technical
  code, diagnostic, raw error, or payload.
- AC-3: Telemetry is grouped and collapsed; system events are discrete.
- AC-4: `Enter` on the selected expandable item toggles only its detail and does
  not execute any daemon-side action.
- AC-5: Permission/clarification/attention items remain visibly actionable and
  are never hidden inside a routine telemetry/tool group.

Edge cases:

- EC-1: A tool has no useful scalar summary → show its name and state only; do
  not expose arbitrary JSON merely to fill the row.
- EC-2: Tool output fails serialization/rendering → show an actionable summary
  and retain raw source access in debug mode.

### US-005: Condense routine events without losing evidence

**As an** Incident Investigator, **I want** consecutive compatible events
condensed with an exact count and reversible expansion, **so that** long runs
stay readable without sacrificing auditability.

Acceptance criteria:

- AC-1: A group states its exact source-member count.
- AC-2: Expanding the group yields every member once, in original order.
- AC-3: Groups never cross user/assistant, error, permission, clarification, or
  attention boundaries.
- AC-4: Global debug mode bypasses summaries/groups and presents raw entries and
  parts in source order, including full command/JSON/path payloads.
- AC-5: Toggling normal/debug mode changes presentation only; raw
  `TranscriptState`, sequences, fences, and pagination are unchanged.

Edge cases:

- EC-1: Two consecutive events look similar but have incompatible categories
  or terminal states → they remain separate.
- EC-2: A group receives another compatible streaming event → count increases
  without changing the order or identity of existing members.
- EC-3: A raw payload is too large for one viewport → it is pageable/wrapped,
  not discarded or silently truncated.

### US-006: Scroll safely while new updates arrive

**As a** Conversation Operator, **I want** scrolling up to suspend follow-tail
and show how many updates arrived, **so that** live work does not pull me away
from what I am reading.

Acceptance criteria:

- AC-1: Any upward navigation away from the bottom disables follow-tail.
- AC-2: Incoming updates while follow-tail is disabled do not change the visible
  anchor/selection and increment a visible unseen-update count.
- AC-3: `G` moves to the latest item, reenables follow-tail, and clears the
  unseen count.
- AC-4: While follow-tail is enabled, incoming updates keep the latest content
  visible.

Edge cases:

- EC-1: A condensed group grows while off-tail → unseen count reflects newly
  received source updates, not merely newly allocated presentation rows.
- EC-2: A reset/refetch occurs while off-tail → preserve the nearest surviving
  source anchor and explain resynchronization; do not falsely report zero new
  updates if newer source sequences exist.

## Adaptive Layout

### US-007: Keep the transcript primary at every usable size

**As a** Terminal Operator, **I want** panel allocation to follow width and
relevance, **so that** empty lists do not crowd the conversation.

Acceptance criteria:

- AC-1: Narrow terminals show exactly one content panel at a time and default to
  transcript when session detail exists.
- AC-2: Medium terminals show transcript plus one contextual rail; empty peers
  compact and the most relevant contextual panel grows.
- AC-3: Wide terminals keep transcript at least 60% of content width and share
  the remainder deterministically by relevance.
- AC-4: Pending attention outranks focus, active work, populated inactive, and
  empty state when allocating contextual space; ties use stable panel order.
- AC-5: Resize preserves focused source item and makes every panel reachable by
  keyboard.

Edge cases:

- EC-1: All contextual panels empty → they remain compact rather than splitting
  the screen equally.
- EC-2: Attention appears while reading a long transcript → it becomes visible
  without replacing the transcript or losing the reading anchor.
- EC-3: Below `80x24` → existing too-small surface appears; no partial layout.

## Workspace Startup

### US-008: Select workspace by deterministic precedence

**As a** Workspace Starter, **I want** explicit selectors and cwd inference to
follow one precedence order, **so that** startup never chooses a surprising
workspace.

Acceptance criteria:

- AC-1: Non-empty `--workspace` wins over all other sources.
- AC-2: Without the flag, non-empty `COMPOZY_WORKSPACE` wins over cwd.
- AC-3: Without either explicit source, canonical cwd matches canonical roots by
  longest path prefix.
- AC-4: Without a match, startup opens onboarding/picker rather than exiting
  with only a generic error.
- AC-5: Invalid explicit selectors remain actionable errors and do not fall
  through to cwd or picker silently.

Edge cases:

- EC-1: Nested registered roots both contain cwd → deepest canonical root wins.
- EC-2: cwd or root includes symlinks → comparison uses canonical paths.
- EC-3: duplicate explicit names → report ambiguity and request id/path.

### US-009: Confirm an unregistered directory before adding it

**As a** Workspace Starter, **I want** to review a derived name and canonical
path before registration, **so that** no workspace is created silently or for
the wrong directory.

Acceptance criteria:

- AC-1: Onboarding shows the derived directory name and full canonical path.
- AC-2: It offers add this directory, choose an existing workspace, and exit.
- AC-3: Add opens a separate confirmation that repeats name and canonical path;
  only confirmation emits a write.
- AC-4: Cancel, choose, and exit do not register, alter config, or affect daemon
  sessions/runs.

Edge cases:

- EC-1: Canonical cwd has no usable final component → display name `workspace`
  while retaining the full path.
- EC-2: Canonicalization fails → do not offer add; show cause plus choose/exit.

### US-010: Enter the workspace after successful registration

**As a** Workspace Starter, **I want** confirmed registration to complete the
normal boot automatically, **so that** I do not have to restart batuta.

Acceptance criteria:

- AC-1: Registration goes through a generic `compozy-client` operation with no
  Ratatui or batuta-specific names.
- AC-2: After success, the TUI refetches the catalog, selects by canonical-root
  equality, and runs the normal panel/stream boot sequence.
- AC-3: A concurrent creator that registers the same canonical root is treated
  as success after refetch, not as a duplicate selected workspace.
- AC-4: Success is shown only after refetch, selection, and boot succeed.

Edge cases:

- EC-1: Add returns success but the refetched catalog lacks the root → show the
  failed phase and offer refresh/choose/exit; do not auto-repeat add.
- EC-2: Boot fails after selection → retain selected identity and actionable
  error without claiming a completed onboarding.

### US-011: Recover without hidden mutation

**As a** Workspace Starter, **I want** unsupported and failed registration to
leave me in control, **so that** I can take an explicit next action.

Acceptance criteria:

- AC-1: A daemon without registration support shows
  `compozy workspace add <canonical-path>` as a copyable fallback.
- AC-2: The TUI never executes that command as a subprocess.
- AC-3: Other daemon errors show human cause/action first and technical details
  second, preserving retry, choose, and exit actions.
- AC-4: Retry requires a new explicit confirmation/write action; errors do not
  trigger background mutation.

Edge cases:

- EC-1: Daemon becomes unavailable during add → candidate stays visible and no
  success is claimed.
- EC-2: Operator cancels after an error → no additional write, local config
  change, session stop, or loop cancellation occurs.
