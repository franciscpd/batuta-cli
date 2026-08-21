# Spec: UX Foundation

Product and technical contract for making batuta-cli calm, readable, and
trustworthy during long-running conversational work. This spec refines the
approved terminal design in
`docs/internal/specs/2026-08-17-batuta-cli-design.md`; it does not replace the
daemon protocol, the batuta-first preset, or the existing safety invariants.

---

# Part I — Product

## Overview

The current TUI exposes the right CompozyOS primitives, but its presentation
still gives transport details, tool payloads, and fixed panel geometry nearly
the same visual weight as the user/assistant conversation. Workspace startup
also ends in an error when the current directory is not registered, despite
having enough context to offer a safe, explicit next step.

UX Foundation makes the transcript the primary reading surface, introduces a
semantic and terminal-compatible visual language, makes layout respond to
content relevance, and turns an unregistered current directory into a
confirmation-gated onboarding flow. Raw daemon data remains available and
lossless; presentation becomes quieter without hiding evidence.

## Approved Requirements (Verbatim)

The following scope is preserved literally from the approved design
conversation and is normative:

1. linguagem visual sóbria/profissional com texto herdado do terminal, tokens semânticos, ciano/azul ativo, verde sucesso, âmbar espera/atenção, vermelho erro/destrutivo, roxo discreto sistema, nenhum estado apenas por cor, compatibilidade 16 cores, temas claro/escuro e NO_COLOR;
2. transcript conversation-first, usuário/assistente completos, tools resumidas e expansíveis, sistema discreto, telemetria agrupada/recolhida, erros humanos antes de detalhes técnicos, Enter expande item, modo debug global preserva o bruto, condensar eventos consecutivos, recolher comandos/JSON/caminhos, wrap, follow-tail suspenso ao scroll e indicador de novas atualizações, estados pensando/tool/aguardando/concluído/falhou;
3. layout adaptativo: transcript primário, painéis vazios compactos, painéis relevantes crescem, terminal estreito mostra um painel por vez;
4. workspace resolution: --workspace, COMPOZY_WORKSPACE, cwd longest-prefix, depois onboarding/picker; diretório não registrado mostra nome derivado+caminho canônico e ações adicionar/escolher/sair; confirmação explícita; generic compozy-client API only, no ratatui/batuta names; após add recarrega catálogo/seleciona/boota; daemon sem operação mostra comando `compozy workspace add` como fallback, sem subprocesso oculto; erros permanecem acionáveis; cancelar/sair não muta;
5. fora de escopo: theme editor, custom palettes, mouse, decorative animation, silent registration.

The required validation matrix is also normative: dark/light/NO_COLOR, 16
colors, narrow/medium/wide, and empty/short/long/tool/error/attention. Test
cases must cover semantic rendering, grouping count/losslessness, debug
preservation, wrapping, follow-tail, layout compaction, precedence, cwd
detection, confirmation, success/error/unsupported fallback/no mutation.

## Goals

- Make user and assistant content the uninterrupted primary narrative.
- Preserve every source transcript entry and raw payload while presenting
  routine operational detail progressively.
- Give every status a stable semantic token plus text and/or a glyph, so color
  is reinforcement rather than the only signal.
- Render predictably in dark terminals, light terminals, 16-color terminals,
  and with `NO_COLOR`.
- Keep the transcript dominant across usable terminal widths and reclaim space
  from empty or irrelevant panels.
- Resolve the active workspace deterministically and offer explicit,
  reversible onboarding when the current directory is not registered.
- Keep all daemon protocol behavior generic in `compozy-client` and all
  rendering pure in `batuta-tui/src/views/`.

## User Stories

Full catalog: [User stories](_user_stories.md).

- US-001–US-002: semantic visual language and accessible terminal modes.
- US-003–US-006: conversation-first transcript, progressive disclosure,
  lossless grouping, follow-tail, and agent activity states.
- US-007: adaptive transcript-primary layout.
- US-008–US-011: deterministic workspace resolution and safe onboarding,
  including success, error, unsupported-daemon fallback, and no-mutation exits.

## Core Features

### Semantic visual language

Normal prose inherits the terminal foreground and background. Semantic tokens
name intent rather than a concrete color: `text`, `muted`, `active`,
`success`, `waiting`, `error`, `destructive`, and `system`. The 16-color
palette maps active to cyan/blue, success to green, waiting/attention to
yellow/amber, error/destructive to red, and system to a restrained
magenta/purple. Every colored state also has a stable label, glyph, border, or
modifier.

`ui.theme = "auto" | "dark" | "light"` selects contrast treatment. `auto`
reads the final numeric `COLORFGBG` component: `0` or `8` selects dark, `7` or
`15` selects light, and absent/invalid/other values select dark. Existing
`ui.color = "auto" | "never"` remains the
color enablement control. `NO_COLOR` always wins and produces no foreground or
background color attributes while preserving labels, glyphs, emphasis, and
layout. No RGB or 256-color-only token is required.

### Conversation-first transcript

User and assistant messages render in full and in source order. System entries
are visually quiet. Tool calls show a one-line summary and state by default;
commands, JSON, paths, input, output, and technical errors stay collapsed until
the selected item is expanded with `Enter`. Human-readable errors appear before
their technical details. Telemetry is grouped and collapsed by default.

Two or more consecutive compatible operational events are condensed into one
presentation group with an exact item count. Grouping never mutates
`TranscriptState`, crosses a user/assistant boundary, absorbs an error or
attention request, or drops ordering/provenance. Expanding a group reveals all
members; global debug mode renders the original entries and parts in source
order with their raw serialized payloads.

Long prose wraps to the available Unicode display width. Long unbroken tokens
soft-wrap rather than disappearing off-screen. When the operator scrolls away
from the tail, follow-tail stops immediately; incoming updates do not move the
viewport and an explicit new-update count appears. Returning to the bottom
with `G` resumes follow-tail and clears the count.

The active turn always communicates one of: `thinking`, `tool`, `waiting`,
`completed`, or `failed`. Each has visible text and a non-color-only marker.

### Adaptive layout

The transcript is the primary surface. Empty panels collapse to compact
headers/status rows. A panel with attention, an active run, or current focus
receives more room than an empty or inactive peer. Allocation is deterministic
for the same model state.

- Narrow (`80–99` columns, at least `24` rows): one panel at a time; the
  transcript is the default when a session is open, and existing focus keys
  switch panels.
- Medium (`100–139` columns): transcript plus one contextual rail; empty peer
  panels remain compact and the most relevant panel grows.
- Wide (`140+` columns): transcript remains at least 60% of content width;
  contextual panels share the remainder according to relevance.
- Below `80x24`: retain the approved too-small screen; this delivery does not
  lower the minimum usable size.

### Workspace resolution and onboarding

Workspace selection follows this exact precedence:

1. non-empty `--workspace`;
2. non-empty `COMPOZY_WORKSPACE`;
3. canonical current directory matched against canonical registered roots by
   longest path prefix;
4. onboarding/picker.

An invalid explicit selector is an actionable error and does not silently fall
through to a lower-precedence source. If no registered root contains the
canonical current directory, startup shows an onboarding surface with a name
derived from the directory's final normal component, the full canonical path,
and exactly three actions: add this directory, choose an existing workspace,
or exit. Adding requires a separate explicit confirmation showing the same
name and canonical path.

If the daemon exposes workspace registration, the TUI calls it through a
generic `compozy-client` API. On success it refetches the workspace catalog,
selects the workspace whose canonical root matches the requested path, then
runs the normal workspace boot sequence. If registration is unsupported, the
surface shows an exact `compozy workspace add <canonical-path>` fallback and
does not spawn a subprocess. Errors keep the operator on the onboarding
surface with cause and next action. Cancel and exit perform no daemon write and
do not change local configuration.

## Business Rules

- Presentation compaction is reversible and lossless; source transcript data
  remains authoritative.
- User and assistant message bodies are never summarized, grouped away, or
  truncated by normal presentation.
- Error and attention entries are never absorbed into a collapsed group.
- `Enter` expands the selected expandable transcript item; it does not execute
  a tool, answer attention, or mutate daemon state.
- Debug mode is global for the running TUI and changes presentation only.
- `NO_COLOR` has precedence over configured theme/color choices.
- No state is conveyed by color alone.
- Explicit workspace selectors have precedence over cwd inference; invalid
  explicit input fails visibly rather than selecting something else.
- Workspace registration never occurs before confirmation and never occurs as
  a side effect of picker navigation, cancel, or quit.
- Successful registration is not considered complete until catalog refetch,
  canonical-root selection, and normal panel/stream boot have succeeded.
- Quitting the TUI never stops or cancels a session or loop run.

## User Experience

Detailed terminal behavior, layouts, tokens, and state examples are defined in
[UI/UX contract](_uiux.md). Operator and developer-facing configuration,
workspace resolution, and error journeys are defined in
[Developer experience](_dx.md).

Primary journeys:

1. Read a long session as a conversation, expanding only the tool/error detail
   needed for the current investigation.
2. Scroll up while work continues, see an accumulating update count without a
   viewport jump, then press `G` to return live.
3. Use the same status semantics with a dark theme, light theme, a 16-color
   terminal, or `NO_COLOR`.
4. Resize from wide to narrow and retain access to every panel while the
   transcript stays primary.
5. Launch from an unregistered directory, review its canonical identity,
   confirm registration, and enter the normal workspace view; or choose/exit
   without mutation.

## High-Level Technical Constraints

- `crates/compozy-client` remains generic, contains no `ratatui` imports and no
  batuta-specific names.
- `crates/batuta-tui/src/views/` performs no I/O and imports no
  `compozy_client`; views render only from `Model`.
- Raw transcript entries remain stored in the existing fenced
  `TranscriptState`; grouping is a derived presentation model/cache.
- Workspace registration uses only a daemon API represented in
  `compozy-client`; unsupported daemons return a typed capability outcome.
- The TUI never shells out to `compozy workspace add`.
- Existing SSE ordering, reset, pagination, and quit invariants remain intact.
- Contract tests must obey the disposable detached-worktree and temporary
  `COMPOZY_HOME` rule.

## Non-Goals (Out of Scope)

- Theme editor.
- Custom palettes.
- Mouse support.
- Decorative animation.
- Silent workspace registration.
- Changing daemon transcript schemas or event order.
- Persisting a second, transformed transcript.
- Replacing the existing keyboard navigation model.
- Lowering the `80x24` minimum terminal size.

## Open Questions

None. Theme selection, grouping losslessness, responsive breakpoints,
workspace precedence, confirmation, unsupported-daemon behavior, and
post-registration boot are resolved in this spec.

---

# Part II — Technical

## Executive Summary

This delivery separates authoritative data from presentation. A semantic
theme maps intent to terminal-safe styles; a derived transcript presentation
layer turns raw entries into reversible rows/groups; a deterministic layout
policy allocates space from width and panel relevance; and a workspace startup
state machine resolves explicit inputs, canonical cwd, picker, confirmation,
registration capability, refetch, selection, and boot. No new crate is needed.

## MVP Boundary

All eleven user stories are MVP. Everything in Non-Goals remains deferred.
There is no partial mode in which registration is silent, grouping is lossy,
or color is the only state signal.

## Developer Experience

- [Developer experience contract](_dx.md)
- [UI/UX contract](_uiux.md)
- [Test specification](_tests.md)

## System Architecture

- **`crates/compozy-client`**: generic workspace registration request/response
  types and a typed `Unsupported` result for absent daemon operation. It owns
  HTTP/UDS protocol details only.
- **`crates/batuta`**: resolves `--workspace`, `COMPOZY_WORKSPACE`, and
  canonical cwd; supplies an unresolved-directory candidate to the TUI rather
  than exiting; retains CLI/config ownership.
- **`crates/batuta-tui` model/update**: owns semantic theme mode, debug mode,
  derived transcript rows, unseen-update count, relevance scores, and the
  onboarding state machine. `update()` emits generic effects.
- **`crates/batuta-tui/src/views/`**: renders semantic styles, transcript rows,
  adaptive areas, onboarding, confirmation, and actionable errors from
  `Model` only.

Data flow remains one-way:

```
daemon/raw transcript -> compozy-client -> Msg -> Model/raw state
                                              -> derived presentation -> view

CLI/env/cwd -> resolver -> selected workspace -----------------------> boot
                         -> onboarding -> confirm -> generic API -> refetch
```

## Core Interfaces

Illustrative ownership, not mandatory Rust spelling:

```rust
enum SemanticToken {
    Text, Muted, Active, Success, Waiting, Error, Destructive, System,
}

enum ThemeVariant { Auto, Dark, Light }

enum TranscriptRow {
    Message { source: EntryKey },
    ToolSummary { source: PartKey, expanded: bool },
    Group { kind: GroupKind, members: Vec<PartKey>, expanded: bool },
    Error { source: PartKey, expanded: bool },
    Activity { state: ActivityState },
}

enum ActivityState { Thinking, Tool, Waiting, Completed, Failed }
```

```rust
enum WorkspaceStartup {
    Resolving,
    Selected(WorkspaceRef),
    Unregistered(WorkspaceCandidate),
    ConfirmAdd(WorkspaceCandidate),
    Adding(WorkspaceCandidate),
    Unsupported { candidate: WorkspaceCandidate, command: String },
    Failed { candidate: WorkspaceCandidate, message: String, action: String },
}
```

The generic client surface accepts only generic workspace data through
`add_workspace(AddWorkspaceRequest) -> Result<AddWorkspaceOutcome, Error>`.
A daemon `404`/`405` for the registration operation maps to
`AddWorkspaceOutcome::Unsupported`; other structured daemon errors retain
their envelope. Neither the type nor module may mention batuta or Ratatui.

## Presentation Invariants

- Each derived row contains stable references back to one or more source entry
  keys/part positions.
- The concatenated expanded members of all groups equal the original eligible
  members in source order and count.
- Group eligibility is limited to adjacent operational parts of the same
  category and compatible state; message, error, permission/clarification,
  and attention boundaries terminate a group.
- Debug presentation bypasses grouping/summarization and serializes the raw
  source entry/part. Large payloads are paged for rendering performance but
  are not truncated or discarded.
- Cache keys include width, theme variant, color enablement, debug state,
  expansion state, and source sequence so resize/mode changes cannot reuse a
  stale render.

## Workspace Resolution Algorithm

1. Load the daemon catalog once.
2. If `--workspace` is present and non-empty, resolve it by existing explicit
   id/name/absolute-path rules; return its result or actionable error.
3. Else if `COMPOZY_WORKSPACE` is present and non-empty, apply the same explicit
   rules; return its result or actionable error.
4. Else canonicalize cwd and every usable registered root; choose the matching
   root with the greatest component count.
5. If no root matches, create a candidate from canonical cwd and its final
   normal path component, then open onboarding. If a usable derived name cannot
   be produced (for example filesystem root), use `workspace` as the displayed
   candidate name and keep the canonical path visible.
6. Choosing an existing workspace opens the catalog picker. Exiting or
   cancelling returns without writes.
7. Confirmed add calls the generic client operation once. Success always
   refetches; a same-root workspace created concurrently is accepted during
   refetch. Select by canonical-root equality and execute the existing full
   workspace boot. Unsupported returns the fallback command; other errors stay
   actionable in onboarding.

## Layout Policy

`views/layout.rs` remains a pure function of terminal area and model-derived
relevance. Relevance is ordered: pending attention > focused panel > active
non-terminal run/session > populated inactive panel > empty panel. Ties use the
stable panel order. Empty contextual panels use only a header/status row where
the width mode permits multiple surfaces. Narrow mode renders exactly one
content panel and does not keep an invisible side column.

## Impact Analysis

| Component | Impact | Required outcome |
| --- | --- | --- |
| `batuta-tui/src/theme.rs` | semantic expansion | terminal-inherited text, dark/light variants, 16-color-only mappings, `NO_COLOR` precedence |
| transcript model/cache/views | presentation refactor | full messages, reversible groups, raw debug, wrapping, unseen updates |
| `batuta-tui/src/views/layout.rs` | policy change | narrow/medium/wide adaptive allocation and compact empty panels |
| `batuta-tui` onboarding model/update/view | additive | candidate, confirmation, add/picker/exit, actionable result states |
| `batuta/src/workspace.rs` | resolution change | strict source precedence and unresolved cwd candidate |
| `compozy-client/src/workspaces.rs` and generic types | additive | daemon registration when supported; typed unsupported result |
| config parsing/docs | additive | `ui.theme = auto|dark|light`; existing `ui.color` and `NO_COLOR` semantics preserved |

## Failure Handling

- Invalid `--workspace` or `COMPOZY_WORKSPACE`: name the source and invalid
  value, explain accepted selector forms, do not infer cwd.
- Canonicalization failure: show the path and OS cause; offer choose or exit,
  but not add an unverified path.
- Registration unsupported: show `compozy workspace add <canonical-path>` and
  explain that the command must be run separately; offer refresh/choose/exit.
- Registration rejected: retain the candidate and confirmation context, show
  the daemon's human message first and diagnostic/code second, allow retry.
- Registration succeeds but refetch/select/boot fails: do not claim success;
  show the failed phase and allow refresh/choose/exit without repeating add
  automatically.
- Transcript presentation failure for an unknown part: render a discrete
  unknown-part summary and keep it available in debug mode; never panic.
