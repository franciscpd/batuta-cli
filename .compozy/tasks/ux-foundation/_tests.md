# Test Specification: UX Foundation

Canonical validation contract for UX Foundation. Derived from
`_user_stories.md` (behavior), `_uiux.md` (rendering/interaction), `_dx.md`
(configuration and operator journeys), and `_spec.md` Part II (ownership and
state transitions).

## Strategy

- **Unit/render tests**: Rust `#[test]` table tests for resolution, grouping,
  wrapping, state transitions, config, and pure view behavior. Ratatui
  `TestBackend` plus `insta` snapshots validate terminal rendering.
- **Integration tests**: `wiremock`/fake transport for generic workspace API
  outcomes and the existing pure `Model`/`Cmd` harness for onboarding through
  boot. No test substitutes a subprocess for daemon registration.
- **E2E tests**: PTY-based TUI journeys for resize, scroll/follow, debug, and
  onboarding. Real-daemon cases use a disposable detached worktree without
  `.compozy/` and a temporary `COMPOZY_HOME`.
- **Naming**: new Rust tests use the reserved `ut_700_...`, `it_700_...`, and
  `e2e_700_...` ranges to avoid collision with existing test IDs.
- **Losslessness oracle**: compare stable source keys plus raw serialized part
  values before grouping, after full group expansion, and in debug mode.

## Required Render Matrix

The canonical snapshot matrix is the full cross-product below, not a sampled
subset: `3 themes × 3 widths × 6 transcript fixtures = 54 snapshots`.

| Dimension | Values / fixture contract |
| --- | --- |
| Theme | `dark`, `light`, `NO_COLOR` |
| Capability | ANSI 16 colors only for every colored snapshot; no RGB/indexed >15 |
| Narrow | `90x30`, exactly one content panel |
| Medium | `120x40`, transcript + one grown contextual rail + compact empty peers |
| Wide | `180x50`, transcript >=60% content width |
| Empty | no transcript entries; compact empty context panels |
| Short | one user and one assistant message |
| Long | markdown, Unicode, indentation, whitespace wraps, and one long unbroken token |
| Tool | consecutive tools, commands, JSON, paths, telemetry, collapsed and expanded variants |
| Error | human message/action plus expandable code/diagnostic/raw payload |
| Attention | permission or clarification that breaks grouping and remains actionable |

`NO_COLOR` snapshots assert absence of foreground/background styles in addition
to text snapshots. Dark/light snapshots inspect every emitted color and fail if
it is neither `Reset` nor an ANSI base/bright 16-color value.

## Coverage Matrix

| Requirement | Unit/render | Integration | E2E |
| --- | --- | --- | --- |
| Semantic rendering and terminal-inherited text | UT-700–UT-705, UT-732 | — | E2E-700 |
| Dark/light/NO_COLOR and 16 colors | UT-700–UT-705, UT-732 | — | E2E-700 |
| Complete conversation-first messages | UT-706, UT-732 | — | E2E-701 |
| Tools/telemetry/system progressive disclosure | UT-707–UT-708, UT-732 | — | E2E-701 |
| Group count, boundaries, order, and losslessness | UT-709–UT-711 | IT-700 | E2E-701 |
| Global raw debug preservation | UT-712–UT-713 | IT-700 | E2E-701 |
| Unicode/long-token wrapping | UT-714–UT-715, UT-732 | — | E2E-702 |
| Follow-tail suspension/new count/resume | UT-716–UT-718 | IT-701 | E2E-702 |
| Thinking/tool/waiting/completed/failed | UT-719, UT-732 | — | E2E-700 |
| Narrow/medium/wide adaptive compaction | UT-720–UT-724, UT-732 | IT-702 | E2E-703 |
| `--workspace`/env/cwd precedence | UT-725–UT-727 | — | E2E-704 |
| Canonical cwd longest-prefix detection | UT-727 | — | E2E-704 |
| Candidate identity and explicit confirmation | UT-728–UT-729 | IT-703 | E2E-705 |
| Generic client boundary | UT-730 | IT-704–IT-705 | — |
| Registration success/refetch/select/boot | — | IT-704, IT-706–IT-707 | E2E-705 |
| Error remains actionable | UT-731 | IT-708–IT-710 | E2E-706 |
| Unsupported command fallback/no subprocess | UT-731 | IT-705 | E2E-706 |
| Cancel/choose/exit no mutation | UT-729 | IT-703, IT-711 | E2E-707 |

## Unit and Render Tests

### Semantic theme

- **UT-700**: normal prose in dark and light variants emits terminal default
  foreground/background (`Reset`/unset), never hard-coded white/gray/black.
- **UT-701**: table-test every semantic token in dark and light: active maps to
  cyan/blue, success green, waiting yellow, error/destructive red, system
  magenta; all colors are within ANSI 16.
- **UT-702**: every state token also renders the required label/glyph/focus
  treatment; removing style attributes leaves states textually distinct.
- **UT-703**: config accepts `ui.theme = auto|dark|light`; any other value
  returns the exact `_dx.md` error with config path and line.
- **UT-704**: `NO_COLOR` overrides configured dark/light and `ui.color=auto`;
  rendered styles contain no foreground/background color while content,
  ordering, and layout equal the colored model.
- **UT-705**: `theme=auto` maps final numeric `COLORFGBG` values `0`/`8` to dark
  and `7`/`15` to light; absent, invalid, and every other value resolve to dark,
  and the result remains stable across renders.

### Transcript hierarchy and disclosure

- **UT-706**: source entries containing multiline user/assistant markdown
  render every source text scalar once and in source order in normal mode; no
  summary or ellipsis replaces either body.
- **UT-707**: collapsed tool fixture renders name/state/safe scalar or basename
  but omits full command, absolute path, JSON, input, and output; `Enter`
  expands only that selected tool and a second `Enter` collapses it.
- **UT-708**: error fixture renders human cause/action before code, diagnostic,
  and raw payload; technical fields appear only after expansion. Permission and
  clarification rows remain visible/actionable in both states.

### Grouping and debug fidelity

- **UT-709**: six adjacent compatible tool updates produce one group labeled
  with count `6`; appending a seventh compatible source update changes the
  count to `7` without changing existing member identities.
- **UT-710**: expanding groups and concatenating member source keys/raw values
  exactly equals the eligible raw source slice in length, order, and value.
- **UT-711**: table-test every group boundary: user, assistant, error,
  permission, clarification, attention, category change, and incompatible
  terminal state each prevent cross-boundary grouping.
- **UT-712**: global debug on a fixture containing tool command, nested JSON,
  Unicode path, system event, telemetry, and error renders each original entry
  and part in source order with byte-equivalent serialized raw values and no
  `truncated` marker.
- **UT-713**: toggling debug normal → raw → normal changes no raw entry,
  sequence, fence, pagination cursor, selection source key, or daemon command;
  only presentation/cache state changes.

### Wrapping and follow-tail

- **UT-714**: Unicode display-width wrapping at narrow/medium/wide widths never
  exceeds its area, preserves continuation indentation, and reconstructs the
  original conversational text when visual line breaks are removed.
- **UT-715**: one token wider than the content area hard-wraps into visible
  chunks with no dropped or duplicated characters.
- **UT-716**: `k`, `Up`, `PageUp`, or `g` away from the latest source item sets
  follow false and retains a stable source anchor.
- **UT-717**: with follow false, N incoming raw updates leave selection/anchor
  unchanged and show `N new updates`; growth of an existing presentation group
  counts raw updates, not new rows.
- **UT-718**: `G` selects the newest source item, sets follow true, clears unseen
  count in the same update, and later deltas remain visible at the tail.
- **UT-719**: table-test `thinking`, `tool`, `waiting`, `completed`, and `failed`
  model states; each produces exactly one matching label, required marker, and
  semantic token, including `NO_COLOR`.

### Adaptive layout

- **UT-720**: `90x30` with session detail renders exactly one non-zero content
  area, Detail; focus keys switch to each list without allocating a hidden rail.
- **UT-721**: `120x40` renders Detail plus one grown contextual panel and only
  compact rows for empty peers.
- **UT-722**: `180x50` assigns at least 60% of content width to Detail and
  distributes the remainder by relevance with stable tie order.
- **UT-723**: table-test relevance ordering: pending attention > focused >
  active non-terminal > populated > empty.
- **UT-724**: all-empty contextual panels consume only compact headers in
  medium/wide; adding attention grows Attention without losing the transcript
  source anchor or focused item across re-layout.

### Workspace resolution and onboarding

- **UT-725**: precedence table: flag+env+cwd selects flag; env+cwd selects env;
  cwd-only selects longest prefix; no match returns onboarding candidate.
- **UT-726**: invalid/ambiguous flag and invalid/ambiguous env return errors
  naming their source and value; resolver does not inspect/select cwd afterward.
- **UT-727**: canonical cwd under two nested registered roots selects the root
  with greatest component count; symlinked cwd/root compare by canonical path;
  sibling string prefixes do not match (`/work/app` vs `/work/application`).
- **UT-728**: candidate derives name from final normal canonical component and
  displays full canonical path; filesystem root falls back to name `workspace`;
  canonicalization failure disables add and preserves choose/exit.
- **UT-729**: opening onboarding, selecting add, cancelling confirmation,
  opening picker, cancelling picker, and exiting emit zero write commands and
  do not alter config/session/run state; only confirmed `Enter` emits one add.
- **UT-730**: boundary check asserts `compozy-client` has no `ratatui`
  dependency/import and its workspace registration module/types contain no
  case-insensitive `batuta`; TUI views have no I/O/client imports.
- **UT-731**: unsupported state renders a command beginning exactly
  `compozy workspace add` with safely escaped canonical argument, exposes
  refresh/choose/exit, and contains no effect/command variant that spawns a
  subprocess. Generic error renders human cause before code/diagnostic.

### Full visual matrix

- **UT-732**: generate and approve all 54 snapshots in Required Render Matrix.
  Each snapshot additionally asserts area bounds, source text preservation,
  state labels, and allowed style set. The tool fixture captures collapsed and
  expanded sub-snapshots without multiplying the canonical matrix count.

## Integration Tests

### Transcript model/presentation integration

- **IT-700**: feed snapshot, deltas, groupable events, attention boundary, and
  reset through the real transcript update path; assert normal expanded groups
  and debug mode each reproduce source order/count, while fences/reset semantics
  remain unchanged.
- **IT-701**: receive deltas while the operator is scrolled up; assert stable
  viewport anchor and raw unseen count, then `G` resumes live and subsequent
  streaming text remains visible.
- **IT-702**: resize one populated model wide → medium → narrow → wide; assert
  no panel becomes unreachable, focus/source selection survives, and layout
  matches the deterministic relevance policy at every step.

### Onboarding state machine

- **IT-703**: unresolved canonical cwd opens onboarding; navigate add → cancel,
  picker → cancel, and exit in separate runs; captured HTTP requests contain no
  registration write and no config/session/run mutation.
- **IT-704**: generic client sends the daemon-contract registration request and
  deserializes an added workspace without importing UI concerns; request uses
  the canonical root and confirmed derived name exactly once.
- **IT-705**: registration endpoint returns `404`, `405`, or explicit
  unsupported capability in table cases; generic client returns `Unsupported`,
  TUI renders the fallback command, and process-spawn capture remains empty.
- **IT-706**: successful add triggers catalog refetch, canonical-root selection,
  normal initial panel reads, and streams in that order; onboarding closes only
  after boot succeeds.
- **IT-707**: add races with another creator and returns conflict while refetch
  contains the same canonical root; state machine selects that single workspace
  and boots without a second add.
- **IT-708**: structured validation/server error retains candidate and shows
  human error first with expandable code/diagnostic; no background retry occurs.
- **IT-709**: transport is lost after confirmed add with indeterminate outcome;
  TUI says success is unconfirmed and requires read-only refresh before another
  explicitly confirmed write.
- **IT-710**: add response succeeds but refetch omits the root, and separate boot
  failure case; both name the failed phase, retain refresh/choose/exit, and do
  not repeat registration.
- **IT-711**: after unsupported or error, choose/exit paths make no further
  writes, do not change config, and do not stop/cancel any session or loop run.

## End-to-End Tests

- **E2E-700 — Semantic terminal journey**: run the real TUI against the fixture
  daemon for dark, light, and `NO_COLOR` under a 16-color `TERM`; verify active,
  success, waiting, error, destructive, and system examples retain labels and
  markers, normal text inherits terminal defaults, and no unsupported color is
  emitted.
- **E2E-701 — Conversation/debug journey**: open the tool fixture session;
  verify full user/assistant messages, collapsed tools/telemetry/paths, exact
  group count, visible attention boundary, `Enter` expansion, then global debug
  exposes all original ordered raw parts and returning to normal loses nothing.
- **E2E-702 — Long live transcript journey**: in a narrow TUI, scroll up a long
  Unicode/long-token transcript, stream five updates, verify no viewport jump
  and `5 new updates`, then press `G` and verify wrapped latest content and live
  follow.
- **E2E-703 — Adaptive resize journey**: resize the same populated session
  `180x50 → 120x40 → 90x30`; verify transcript >=60% wide, compact empty panels
  medium, one panel narrow, attention growth, keyboard reachability, and stable
  selected source item.
- **E2E-704 — Resolution journey**: in disposable nested/symlinked directories,
  verify flag > env > canonical longest-prefix cwd; invalid explicit selectors
  remain errors and never select the cwd workspace.
- **E2E-705 — Confirmed registration journey**: launch from an unregistered
  directory against a supporting daemon, verify derived name/canonical path,
  open/cancel confirmation with zero writes, confirm once, then observe catalog
  refetch, selection, and complete normal boot without restart.
- **E2E-706 — Unsupported/error journey**: run against an unsupported daemon
  and a structured-error daemon; verify exact `compozy workspace add` fallback,
  no child process, human-first actionable error, retry only after explicit
  confirmation, and no false success.
- **E2E-707 — No-mutation exits**: exercise onboarding exit, picker cancel,
  confirmation cancel, and exit after error; compare daemon workspaces,
  sessions, loop runs, local config, and captured writes before/after — all are
  unchanged.

## Acceptance Gate

The delivery is not complete unless:

- all 54 canonical render snapshots pass;
- every losslessness comparison passes with exact count/order/value;
- debug mode preserves raw data and emits no daemon mutation;
- all layout breakpoints and follow-tail transitions pass;
- every workspace precedence, canonicalization, confirmation, success, error,
  unsupported, and no-mutation case passes;
- boundary checks preserve generic `compozy-client` and pure views;
- any real-daemon contract run follows the detached disposable-worktree rule.
