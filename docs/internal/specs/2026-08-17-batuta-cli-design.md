# batuta-cli design: a batuta-first terminal UI for CompozyOS

Date: 2026-08-17

## Goal

Give people who live in the terminal a way to run the whole Batuta flow
without opening the CompozyOS web app: talk to the `batuta` agent in a
session, watch spec and tasks appear, follow the `batuta-deliver` Loop live
(nodes, `needs_attention`, quarantine), and answer approvals and
clarifications — all from one keyboard-driven Rust/Ratatui application that
talks to the local CompozyOS daemon.

The product is batuta-first. The architecture is generic: nothing in the
daemon client knows the word "batuta", and every panel is a CompozyOS
primitive (session, loop run, attention, logs). Batuta is a preset of
defaults, so widening the tool into a general CompozyOS TUI later means
changing defaults, not rewriting.

## Non-goals (v1)

- Reimplementing the web window manager (desktops, tiling, tabs). tmux does
  that job for terminal users.
- Loop DAG editor, Settings, onboarding, marketplace install, vault,
  gateway pairing, usage charts. The `compozy` CLI already covers those.
- Remote daemons. v1 speaks only to the local daemon; `compozy connect`
  (device credential + stream tickets) and SSH forwards come later.
- Mouse support beyond scroll, Windows builds, network/memory/agents views.
- Being an official CompozyOS component. Same disclaimer as batuta-compozy.

## Facts about the daemon this design relies on

Verified against `compozy/compozy` at commit `069f8b25` (2026-08-17):

- Two listeners with identical route sets: plain HTTP on `localhost:2123`
  and plain HTTP over the Unix socket `$COMPOZY_HOME/daemon.sock`
  (default `~/.compozy/daemon.sock`, mode 0600). No authentication on the
  local surface; browser-origin protection only fires when `Origin` or
  `Sec-Fetch-Site` headers are present.
- Contract: `openapi/compozy.json` (OpenAPI 3.0.3, 371 paths). Reference
  client: `internal/cli/client*.go`.
- SSE streams: `GET /api/workspaces/{ws}/sessions/{id}/stream`
  (`transcript_snapshot`, `transcript_delta`, `session_stopped`, `done`;
  reconnect with `Last-Event-ID`/`after_sequence` plus `epoch` and
  `generation`; a fence mismatch returns a snapshot with `reset:true`),
  `GET /api/sessions/catalog-stream`, `GET /api/logs/stream` (cursor
  `RFC3339Nano|sequence`), `GET /api/workspaces/{ws}/loop-runs/{id}/events`.
- Transcript entries are Vercel AI SDK `UIMessage` values: `{message:{id,
  role, parts[]}, start_sequence, sequence}` with part types `text`,
  `reasoning`, `dynamic-tool` (`state` in `input-streaming |
  input-available | output-available | output-error`), `file`,
  `data-compozy-permission`, `data-compozy-event`, plus a closed marker
  vocabulary (`prompt_queued`, `provider_failure`,
  `file_mutation_unverified`, ...).
- Approvals (`POST .../sessions/{id}/approve {request_id, turn_id,
  decision}`) and clarifications (`GET .../clarifications`,
  `POST .../clarifications/{id}/answer {choice_index|text}`) block the
  agent until answered.
- Busy sessions accept `mode=queue|steer|interrupt`; `interrupt`/`steer`
  require `expected_turn_id`. Every prompt carries `message_id` and
  `idempotency_key`; a `409` means an indeterminate dispatch and must not
  be retried blindly.
- `GET /api/observe/overview` returns `attention` items whose verbs are
  the only ones the daemon accepts (`approve`, `reject`, `retry`, `open`).
- There is no official TUI. `bubbletea` in `go.mod` only backs the install
  wizard; `compozy session resume` prints a one-shot bundle, it is not a
  REPL. Closing a viewer never cancels an accepted prompt; a TUI quit must
  never stop a session (upstream once shipped that and reverted it).

## Product shape

A single multi-panel application in the lazygit style. Panels keep fixed
positions; focus moves with `Tab` or the panel number; the detail panel
follows the focused list.

```
┌ batuta ─ ws: batuta-compozy ▾ ─ daemon ok ─ 2 attention ───────────────────┐
├─[1] Sessions ────────┬─[4] Session: batuta · "spec login" ────────────────┤
│ > batuta  spec login │ user  ▸ implement login with magic link            │
│   batuta  fix ci     │ batuta                                             │
│   general  review    │   ▶ cy-create-spec  (tool · done)                  │
├─[2] Deliver runs ────┤   ▶ cy-create-tasks (tool · done · 4 tasks)         │
│ ● run 8f2a  running  │   Dispatched batuta-deliver (run 8f2a)…            │
│   run 7c10  succeeded│   ▶ loop_start (tool · running)                    │
│   run 6b91  failed   │ ─ marker: prompt_accepted ─                        │
├─[3] Attention (2) ───┤                                                    │
│ ! approve  Bash rm…  │                                                    │
│ ! clarify  which env?├────────────────────────────────────────────────────┤
│                      │ > _                                   [Enter] send  │
├──────────────────────┴────────────────────────────────────────────────────┤
│ 1-4 focus  j/k move  Enter open  a approve  x reject  w workspace  ? help │
└───────────────────────────────────────────────────────────────────────────┘
```

Panels:

- **[1] Sessions** — session catalog of the active workspace, default
  filter `agent=batuta`; `*` toggles all agents; `n` creates a new session
  with the preset agent.
- **[2] Deliver runs** — loop runs of the workspace, default filter
  `loop=batuta-deliver`; `*` toggles all loops. State shown with a symbol
  and a color, never color alone.
- **[3] Attention** — `observe/overview.attention` items plus pending
  clarifications of visible sessions. Verbs are exactly the daemon's:
  `a` approve, `x` reject, `r` retry, `Enter` open.
- **[4] Detail** — follows focus. Session: transcript + composer. Run:
  node timeline from the events stream with `p` pause, `u` resume,
  `k` kill, `a` approve. Attention item: the item in context.

Transcript rendering: `text` as markdown; `reasoning` collapsed by default
(`t` toggles); tool calls as collapsible cards (`Enter` expands; edit tools
render a diff); `data-compozy-permission` becomes both an inline approval
card and an item in [3]; markers as one dim line.

Composer: `Enter` sends, `Alt+Enter` inserts a newline, `Esc` returns focus
to the transcript. When the session is busy, `Enter` opens a chooser for
`queue` / `steer` / `interrupt`.

Global keys: `w` workspace picker, `/` filter in the focused panel,
`?` help overlay, `L` logs overlay filtered by the focused session or run,
`q` quit. Minimum size 100x30; below 100 columns the left column collapses
to the focused list; below 80x24 the app asks for a bigger terminal.
No mouse capture. 16-color base palette; `NO_COLOR` respected.

## Architecture

Cargo workspace, three crates:

```
batuta-cli/
├── Cargo.toml                     # workspace, edition 2024, pinned MSRV
├── crates/
│   ├── compozy-client/            # daemon client; no UI, no "batuta"
│   │   ├── transport.rs           # UDS (hyper + hyperlocal) or TCP; base URL; timeouts
│   │   ├── sse.rs                 # generic stream: reconnect, cursor, backoff
│   │   ├── sessions.rs            # list/create/prompt/approve/clarify/transcript/stream
│   │   ├── loop_runs.rs           # list/get/events/approve/pause/resume/kill
│   │   ├── observe.rs  logs.rs  status.rs  workspaces.rs
│   │   └── types/                 # serde structs: UIMessage parts, fences, runs, attention
│   ├── batuta-tui/
│   │   ├── app.rs                 # Model + update(Msg) -> (Model, Vec<Cmd>)
│   │   ├── msg.rs                 # Msg: Key, Tick, Resize, Stream(..), Api(..)
│   │   ├── cmd.rs                 # Cmd = effects (Fetch, Post, StartStream, StopStream)
│   │   ├── runtime.rs             # runs Cmds on tokio; feeds Msgs back through one mpsc
│   │   ├── panels/                # sessions, runs, attention, transcript, run_detail, logs, composer
│   │   ├── views/                 # pure render: fn view(&Model, &mut Frame)
│   │   ├── keymap.rs  theme.rs    # semantic color slots over the 16 ANSI colors
│   │   └── preset.rs              # Preset { default_agent, featured_loop, filters, ... }
│   └── batuta/                    # binary: clap; `batuta [--workspace] [--daemon uds|tcp]`, `batuta doctor`
├── contract/compozy-<version>.json  # pinned OpenAPI for drift tests
├── tests/contract/                # real daemon in a temporary COMPOZY_HOME
└── docs/internal/{specs,plans}/
```

Boundaries, enforced in CI:

- `compozy-client` does not depend on `ratatui` and contains no
  batuta-specific names.
- `views/*` never import `compozy_client` and never perform I/O; they read
  `Model` only.
- `update()` is a pure function from `Msg` to a new `Model` plus effects,
  testable without a terminal or a network.

The client is hand-written for the endpoints v1 uses (about 25). The
pinned OpenAPI document is a test oracle, not the client source: the spec
is 11.6 MB with 455 operations and the API is beta, so generating a full
client would be slow to build and brittle to track.

### State model

`Model` holds: `workspace: Option<Workspace>`, `focus: Panel`,
`sessions: ListState<SessionRow>`, `runs: ListState<RunRow>`,
`attention: Vec<AttentionItem>`, `detail: Detail` (one of
`Session(TranscriptState)`, `Run(RunState)`, `Attention(..)`),
`composer: TextArea`, `overlay: Option<Overlay>` (`Help`, `Logs`,
`WorkspacePicker`, `Confirm`), `toast: Option<Toast>`, `daemon: DaemonStatus`.

`TranscriptState` keeps entries indexed by `start_sequence` together with
`epoch`, `generation`, and `max_sequence`. Fences live in state, not in the
stream task, so a reconnect always sends the current values.

`Preset` defaults describe batuta (agent `batuta`, featured loop
`batuta-deliver`, provider/model for the first prompt, theme). Values may be
overridden in `~/.config/batuta/config.toml`.

## Data flow

The daemon is the source of truth. The TUI keeps no local counters and no
shadow lists; a stream either signals "refetch" or delivers fenced deltas.

| Panel | Initial read | Live | Update rule |
|---|---|---|---|
| Sessions | `GET /api/sessions?workspace_id&type=user&sort=last_activity` | SSE `/api/sessions/catalog-stream` | each wake refetches the page of the workspace named in the event |
| Deliver runs | `GET /api/workspaces/{ws}/loop-runs` (filter by loop name server-side if the route supports it, otherwise client-side) | poll every 5 s while a non-terminal run is listed; otherwise on focus or manual refresh | terminal state stops the poll |
| Attention | `GET /api/observe/overview?workspace=` (`attention`) + `GET .../sessions/{id}/clarifications` for visible sessions | catalog-stream wakes; transcript deltas carrying `data-compozy-permission` | any verb executed triggers an overview refetch |
| Detail: session | `GET .../sessions/{id}/transcript` (newest page; scroll up uses `before_sequence` while `has_older`) | SSE `.../sessions/{id}/stream?frames=transcript&epoch&generation&after_sequence` | `transcript_snapshot` with `reset:true` replaces state; `transcript_delta` applies by `start_sequence`/`sequence`; `session_stopped` closes the stream |
| Detail: run | `GET .../loop-runs/{id}` (+ `/turns` on demand) | SSE `.../loop-runs/{id}/events` with `Last-Event-ID` | append to the timeline; a terminal outcome closes the stream |
| Logs overlay | `GET /api/logs?...` | SSE `/api/logs/stream` with the composite cursor | filters (`session`, `run`, `error-only`) come from the focused item |
| Header | `GET /api/status` at boot and every 30 s | — | shows `daemon.status` and version; an unsupported version warns, never blocks |

Writes, each followed by a structured read of the affected resource:

- Prompt: `POST .../sessions/{id}/prompt` with a TUI-generated `message_id`
  and `idempotency_key`; when busy, `mode=queue|steer|interrupt` plus
  `expected_turn_id`. The SSE body of the POST is not consumed; the
  transcript stream already carries the turn.
- New session: `POST /api/sessions` with the preset agent in the active
  workspace; the first prompt selects the preset provider/model.
- Approve: `POST .../sessions/{id}/approve`; clarify:
  `POST .../clarifications/{id}/answer`; loop run:
  `POST .../loop-runs/{id}/{approve,pause,resume,kill}`.

Transport: UDS at `$COMPOZY_HOME/daemon.sock` (default `~/.compozy`), TCP
`localhost:2123` as fallback or by flag. SSE parsed with
`eventsource-stream`; reconnect with exponential backoff (0.5 s to 10 s)
always sending the current cursor and fences; a `reset` shows one dim line
"resynchronized" in the panel.

Concurrency: every stream is a tokio task sending `Msg` values through one
`mpsc` channel into the UI loop; render at most 30 FPS, coalescing deltas
that arrive within one tick; no HTTP on the render thread.

Workspace resolution: `--workspace`, then `COMPOZY_WORKSPACE`, then the
current directory via `GET /api/workspaces/resolve`, then the picker.
Switching workspaces cancels every stream task and re-runs panel boot.

## Errors and edge cases

- Daemon absent: a single screen naming the UDS and TCP targets tried and
  the hint `compozy start`; retry every 3 s; never panic. If the daemon
  dies mid-session the header shows `daemon: offline`, streams stop, panels
  keep their last state dimmed as stale, and everything resumes when the
  daemon returns.
- Version: `GET /api/status` at boot; a version below the supported floor
  (a constant in the binary, like `min_compozy_version` in the batuta
  extension) shows a yellow banner and continues. A `404` on a known route
  reports "route missing in this daemon version" instead of crashing.
- Draining (`503 daemon is draining`): writes show the daemon's message in
  a toast; reads continue.
- HTTP errors keep the daemon's structured `code` and `message`. A `409`
  on prompt means an indeterminate dispatch: toast "check the session"
  and no automatic retry. A fence conflict (`revision`,
  `expected_turn_id`) refetches and asks the user to repeat the action.
- Transcript: `reset:true` replaces the whole state; a delta whose
  `start_sequence` is unknown forces a page refetch; archived entries are
  out of scope. The `file_mutation_unverified` marker renders as a
  highlighted warning because it is a mandatory verification signal.
- Approvals show `turn_id` and `request_id`; a card disappears only when a
  later delta replaces it. `Esc` never answers anything implicitly.
  Clarification deadlines are visible when present.
- Stopped session: the composer still accepts a prompt (the daemon
  restarts the runtime); `session_stopped` closes the stream cleanly and
  the panel says "stopped — sending a prompt restarts it".
- Quit (`q`, `Ctrl+C`): cancel tasks and restore the terminal, nothing
  else. Terminal restoration uses an RAII guard plus a panic hook.
- Small terminals: compact mode below 100x30, refusal below 80x24.
- No color, tmux, SSH: 16-color base, `NO_COLOR`, no mouse capture, works
  over an SSH forward because only UDS/TCP is required.

## Testing

- `compozy-client`: unit tests against a fake HTTP server (`wiremock`)
  per endpoint; SSE parser tests on fixtures captured from a real daemon
  (snapshot, delta, reset, session_stopped); contract tests in
  `tests/contract/` that start a real daemon in a temporary
  `COMPOZY_HOME` (same pattern as batuta-compozy: detached worktree, never
  a checkout containing `.compozy/`) and exercise list, create, prompt with
  cancel, transcript, stream, approve.
- `batuta-tui`: table-driven tests of `update()` (Msg → expected Model and
  Cmds); render tests with `ratatui::backend::TestBackend` and `insta`
  snapshots at 80x24, 120x40, 200x60; a test that the help overlay and the
  keymap table agree.
- Drift: a test loads `contract/compozy-<version>.json` and checks that
  every serde type used by v1 deserializes the schemas and examples of the
  endpoints it consumes.

## CI and release

- `ci.yml`: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`,
  a boundary check (`compozy-client` without `ratatui`; `views` without
  I/O), and a separate contract job that installs the pinned CompozyOS
  release. Conventional commits with `git-cliff`; changes reach `main` only
  through pull requests.
- `release.yml`: `cargo-dist` builds Linux and macOS binaries with
  checksums on `v*` tags; `batuta --version` also prints the supported
  CompozyOS floor.

## Spike before the implementation plan

Two to three days, throwaway unless it comes out clean: a minimal
`compozy-client` (UDS, transcript, stream) and a raw render of a real batuta
session's transcript. It must answer three questions before the MVP plan is
written: does UDS via `hyperlocal` behave; do `transcript_delta` and `reset`
apply without visual glitches; are tool cards and streaming markdown
readable in a terminal.

## Deferred (recorded, not lost)

Remote daemons via `compozy connect`; `batuta tmux up` opening one tmux
window per workspace with the app inside; network, memory, and agents
views; mouse; Windows; a generic (non-batuta) preset selectable by flag;
`batuta` as a marketplace extension contributing a command that launches
the binary.
