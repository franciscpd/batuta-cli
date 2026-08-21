# Spec: Resilience & Release (Delivery 3)

Third and final planned delivery for batuta-cli, per the 3-delivery split
agreed 2026-08-17 (`docs/internal/specs/2026-08-17-batuta-cli-design.md`).
Deliveries 1 (`spike-foundation`) and 2 (`mvp-tui`) are merged into `main`,
tagged `v0.1.0-beta.1`. Tracks GitHub issue #3.

---

# Part I — Product

## Overview

batuta-cli's TUI and CI pipeline currently assume the happy path: the
daemon is always reachable, contract tests are opt-in, and there is no
release automation or public-repo housekeeping (license files, changelog,
a working contributor-docs link). This delivery closes those gaps so the
TUI degrades honestly when the daemon is absent or draining, PRs get
automatic daemon-contract feedback, and the project can cut a trustworthy,
checksummed public release.

- **What problem it solves**: silent crashes and confusing errors when
  the daemon is unreachable or shutting down; contract regressions that
  only surface when someone remembers to run tests manually; no
  repeatable, reviewable way to publish a release.
- **Who it is for**: TUI Operators (daily users of `batuta`), Contributors
  (open PRs, eventually cut releases), and New Adopters (discover the
  project on GitHub and install a release).
- **Why it is valuable**: makes daemon-absence and draining
  self-explanatory instead of alarming, catches breaking daemon-contract
  changes before merge instead of after, and turns "make a release" from
  a manual, error-prone act into a reviewed, automated one.

## Goals

- Operators see an honest, self-recovering retry screen when the daemon
  is unreachable at startup, instead of the TUI failing to launch.
- Operators can tell "daemon draining" apart from "daemon unreachable" at
  a glance, and keep reading in-flight state during a drain.
- The catalog stream recovers on its own after a daemon degrades and
  recovers — no manual restart required to see live updates resume.
- Every PR touching daemon-contract-relevant code gets automatic
  pass/fail feedback from a real daemon, without anyone setting an env
  var or triggering a manual workflow run.
- Cutting a release becomes "merge a reviewed PR," not a sequence of
  manual `cargo publish`/tagging steps.
- New Adopters land on a repo with correct licensing, a working
  contributor-docs link, and a changelog that covers the project's whole
  history, not just what ships from this delivery forward.

## User Stories

Full catalog: [Full user stories](_user_stories.md)

- US-001–US-002 (Startup Resilience): retry screen on daemon-absent
  startup, and manual exit from it.
- US-003–US-004 (Draining UX): distinct draining indicator; reads keep
  working, writes are refused, during a drain.
- US-005 (Stream Recovery): catalog SSE stream self-heals after the
  daemon un-drains.
- US-006 (CI Contract Tests): contract tests run automatically, against a
  real daemon, on every PR.
- US-007–US-008 (Release Pipeline): reviewed release PR gate; checksummed
  binaries on GitHub Releases.
- US-009 (Doctor Reinforcement): `batuta doctor` reports stream health.
- US-010–US-012 (Publish-Prep): dual-license files, working README link,
  full-history changelog.

## Core Features

- **Startup retry screen**: when `batuta` can't reach the daemon at
  launch, it opens the TUI on a retry screen (not a stderr exit),
  retrying indefinitely with the specific connection error shown, and
  transitions automatically once the daemon answers. Quitting from this
  screen never touches daemon-side state (existing invariant, unaffected).
- **Draining as a first-class state**: `draining` becomes a distinct
  `Model` state (not a raw string compared ad hoc), rendered as its own
  banner, separate from the existing offline-dimming behavior. Reads keep
  working; writes are refused with a draining-specific message.
- **Catalog stream self-heal**: fixes the found bug where the catalog SSE
  stream, once it falls back to 10s polling after a 503/draining
  response, never attempts to re-establish SSE — it now recovers once the
  daemon is healthy again, with a bounded retry cadence so daemon flapping
  doesn't cause a reconnect storm.
- **CI contract job**: the existing `contract` job (`.github/workflows/ci.yml`)
  gets a pinned-daemon-install step and runs on every PR, not just
  `workflow_dispatch`/env-var-gated.
- **Release pipeline**: `git-cliff`-driven changelog plus a PR-triggered
  release flow (release-plz style) that opens/updates a standing release
  PR with version bump and changelog diff; merging it tags the version and
  triggers `cargo-dist`'s `release.yml` to build and publish checksummed
  binaries. Full-history changelog generated once, retroactively, covering
  deliveries 1 and 2.
- **`batuta doctor` stream-health check**: doctor gains a check surfacing
  SSE stream health (stale/reconnecting/fatal) additive to its existing
  one-shot checks; exact mechanism (TUI-internal vs. doctor-observable) is
  an Open Question for Part II.
- **Publish-prep housekeeping**: add `LICENSE-MIT`/`LICENSE-APACHE`
  matching `Cargo.toml`'s `license = "MIT OR Apache-2.0"`; repoint the
  README's "Contributing" section at `CONTRIBUTING.md` instead of the
  git-ignored, nonexistent `.compozy/tasks/mvp-tui/_dx.md`; generate
  `CHANGELOG.md` retroactively via `git-cliff`.

## Business Rules

- A `draining` daemon is a distinct lifecycle state from `unreachable`:
  reads are always permitted while draining; writes are always refused
  while draining, surfaced with a draining-specific message (existing
  rule for session-create/prompt writes, extended to be the general rule
  for any write action).
- The startup retry screen retries indefinitely by default — there is no
  automatic give-up; only an explicit user quit ends the retry loop
  (US-002, confirmed over a timeout-based give-up).
- Quitting `batuta` — from the retry screen or from a live session — never
  stops or cancels a session or loop run on the daemon (existing
  project-wide invariant, `CLAUDE.md`; explicitly re-confirmed in scope
  for the new retry screen).
- The CI `contract` job runs on every PR unconditionally once this
  delivery ships; the `workflow_dispatch`/`COMPOZY_TEST_DAEMON_BIN`-only
  gating is removed, not kept as a fallback.
- No tag, build, or publish happens without a human merging the release
  PR — the release gate is a hard rule for this delivery, not a
  configurable default (automating it further is explicitly deferred,
  see Non-Goals).
- Commits that don't follow Conventional Commits format are excluded from
  generated changelog sections (`filter_unconventional = true`), not
  shown as malformed entries.
- `LICENSE-MIT`/`LICENSE-APACHE` presence and content must stay consistent
  with every workspace crate's `Cargo.toml` `license` field — no crate may
  declare a license the repo doesn't carry the text for.

## User Experience

Key personas and their goals are documented in [Full user stories](_user_stories.md#personas).

Primary flows:

1. **TUI Operator, daemon down at launch**: runs `batuta` → sees retry
   screen with specific error → daemon comes up → TUI transitions to
   normal session view automatically. No restart needed.
2. **TUI Operator, daemon drains mid-session**: sees draining banner
   replace the normal header state → keeps browsing sessions/runs/logs →
   attempts a write, sees a draining-specific refusal → daemon finishes
   draining and comes back healthy (new process) → banner clears, catalog
   stream self-heals without a restart.
3. **Contributor, opens a PR**: pushes a branch touching `compozy-client`
   → CI runs `contract` job automatically against a pinned daemon → sees
   pass/fail in the PR checks, same as any other job.
4. **Contributor, cuts a release**: merges feature PRs to `main` → a
   release PR appears/updates automatically with version bump + changelog
   diff → reviews and merges it → tag is created → `release.yml` builds
   and publishes checksummed binaries.
5. **New Adopter, discovers the project**: lands on the GitHub repo →
   sees `LICENSE-MIT`/`LICENSE-APACHE` and a populated `CHANGELOG.md` →
   follows the README's contributing link successfully to `CONTRIBUTING.md`
   → downloads a release binary, verifies its checksum, runs
   `batuta doctor` if setup fails.

This delivery introduces no new UI-bearing web surfaces (it's a terminal
app and CI/release tooling), so there is no `_uiux.md`; TUI-facing
resilience states are covered as `_dx.md`-adjacent behavior and mapped to
components directly in Part II.

Accessibility: unaffected — no new interaction patterns beyond existing
keybind conventions (retry screen reuses the existing quit keybind).

Onboarding/discoverability: the retry screen and draining banner are
self-explanatory by design (US-001 AC-3, US-003 AC-1); no separate
onboarding flow is introduced.

## High-Level Technical Constraints

- Must respect existing crate boundaries (`CLAUDE.md`): `compozy-client`
  stays batuta-agnostic and `ratatui`-free; `batuta-tui/src/views/` keeps
  rendering from `Model` only, no I/O or `compozy_client` imports.
- Contract tests must keep running from a disposable, detached worktree
  without a `.compozy/` directory — the CI job must honor this constraint
  the same way local invocations already document (`compozy-testkit/README.md`,
  `CONTRIBUTING.md`).
- Release binaries target the platforms the design doc already commits to
  (Linux and macOS at minimum) — no new platform commitments are made in
  this delivery.
- **Agent/operator manageability**: `batuta doctor --json` is the existing
  machine-readable diagnostic surface; stream-health reporting extends it
  rather than introducing a separate mechanism, so operators/agents keep
  one place to check daemon+client health.
- **Extension ecosystem expectation**: none — this delivery has no
  extension, skill, or plugin surface; it is internal resilience, CI, and
  release tooling.

## Non-Goals (Out of Scope)

- Windows support — not addressed in any delivery to date; stays out of
  scope here too.
- Telemetry or usage analytics of any kind.
- A plugin/extension system for batuta-cli itself.
- Simultaneous multi-daemon support in one TUI session.
- Further automating the release human-gate (e.g., auto-merging the
  release PR) — the operator explicitly wants the gate manual for now and
  will revisit automation later as a follow-up issue, not as part of this
  delivery.

## Open Questions

None remaining — both items below were resolved during the Stage 2
surface grill and are recorded as decisions, not open questions:

- **Doctor stream visibility (was open)**: resolved — `batuta doctor`
  does not observe another running TUI process. It performs its own
  short-lived probe (2s timeout) of the workspace catalog stream only,
  the one stream checkable without a live session; session-scoped
  streams (transcript, loop events, logs) are omitted from `streams`
  entirely rather than reported as unknown. See `_dx.md`.
- **Retry/backoff cadence (was open)**: resolved — the startup retry
  screen (US-001) retries on a flat 3s cadence, matching the design
  doc's original promise; no exponential backoff, since it is a
  short-lived foreground wait screen, not a background stream. Catalog
  self-heal (US-005) reuses the existing `ReconnectPolicy` exponential
  backoff (0.5s–10s) already used by other SSE streams. See `_dx.md`.

---

# Part II — Technical

## Executive Summary

This delivery is five mostly-independent technical tracks sharing one
theme (honest degradation, real automation): a pre-TUI retry loop for
daemon-absent startup (ADR-001); a `DaemonState` enum consolidating
draining detection currently duplicated as raw-string comparisons
(ADR-002); a one-line root-cause fix to the catalog SSE stream's
`retry_server_errors` flag that removes an entire dead-end fallback path
instead of patching around it; a pinned-daemon CI step using `go install`
(ADR-003); and a hand-assembled, PR-gated release pipeline built from the
two tools the issue names literally — `git-cliff` and `cargo-dist`
(ADR-004) — plus publish-prep housekeeping (LICENSE files, README link,
retroactive CHANGELOG). No new crates are introduced; all work lands
inside the four existing workspace members.

## MVP Boundary

All twelve user stories (US-001–US-012) compose the MVP — this is
Delivery 3, the final planned delivery, and the issue's scope note draws
no post-MVP line within it. Nothing here is post-MVP. Explicitly out of
scope: everything listed in Part I's Non-Goals (Windows support,
telemetry, plugin system, multi-daemon, and automating the release
human-gate — the last one is the one item within "release automation"
that is deliberately deferred, not the pipeline itself).

## Developer Experience

- [Developer experience contract](_dx.md) — covers the CLI surface only:
  `batuta` launch behavior (retry screen) and `batuta doctor`
  human/`--json` output (new `streams` block). No YAML, HTTP/UDS API,
  SDK, or native-tool surface changes; this delivery adds no
  `compozy__*` tools.
- No `_uiux.md` — no Web surface is touched; the TUI is a terminal
  application and its resilience states are covered directly here and in
  `_dx.md`.

## System Architecture

- **`crates/batuta`** (bin crate): gains the pre-TUI retry loop
  (ADR-001) in `app.rs`; `doctor.rs` gains the catalog-stream probe and
  its own `DaemonState`-equivalent draining derivation (ADR-002).
- **`crates/batuta-tui`**: gains `DaemonState` enum in `model.rs`
  consumed by `header.rs` and the dimming call sites; no new `AppMode`,
  no new `TimerId` (the catalog fix removes code rather than adding a
  timer — see Implementation Design).
- **`crates/compozy-client`**: one-line change in `sse/catalog.rs`
  (`retry_server_errors: false → true`); `status.rs` unchanged (keeps
  returning the raw `"draining"` string, per ADR-002).
- **`crates/compozy-testkit`**: unaffected by CI/release changes (those
  live in `.github/workflows/`, not test-harness code); the worktree
  requirement it documents is verified, not modified.
- **CI/release** (`.github/workflows/ci.yml`, new `release.yml`,
  `dist-workspace.toml`, `cliff.toml`): outside the crate graph, but
  designed in this delivery (ADR-003, ADR-004).

Data flow: unchanged for the live-session path (compozy-client SSE →
batuta-tui `Cmd`/update → `Model` → views). The only new data flow is the
pre-TUI retry loop's direct `probe()` call, which is the same call
`main.rs` already makes today, just looped and rendered instead of
one-shot-and-exit.

## Architectural Boundaries

- `compozy-client` continues to import nothing from `ratatui` or
  `batuta-tui`, and gains no batuta-specific types (ADR-002 explicitly
  preserves this).
- `batuta-tui/src/views/` continues to render from `Model` only — the new
  draining banner reads `Model.daemon_state`, performs no I/O.
- `batuta` (bin crate) may depend on `ratatui` directly for the pre-TUI
  loop, exactly as it already does for the main run's terminal setup —
  no new dependency is added, the existing one is used slightly earlier
  in `app.rs`'s control flow.
- No new internal packages/crates are introduced.

## Implementation Design

### Core Interfaces

```rust
// crates/batuta-tui/src/app/model.rs — new enum, single derivation point
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonState {
    Connected,
    Draining,
    Offline,
}

impl DaemonState {
    pub fn derive(status: &DaemonStatus, offline: bool) -> Self {
        if status.status == "draining" {
            DaemonState::Draining
        } else if offline {
            DaemonState::Offline
        } else {
            DaemonState::Connected
        }
    }
}
```

```rust
// crates/batuta/src/app.rs — pre-TUI retry loop (ADR-001)
// Runs before Model/Client construction; owns the terminal directly.
fn await_daemon(
    cli: &Cli,
    terminal: &mut Terminal<impl Backend>,
) -> Result<Client, AppError> {
    let mut attempt: u32 = 0;
    loop {
        attempt += 1;
        match probe(cli) {
            Some(client) => return Ok(client),
            None => {
                terminal.draw(|f| render_retry_screen(f, attempt, &last_probe_error()))?;
                if wait_or_quit(Duration::from_secs(3))? {
                    return Err(AppError::reported(0)); // user quit, not a failure
                }
            }
        }
    }
}
```

```rust
// crates/batuta/src/doctor.rs — additive streams block
struct StreamCheck {
    state: StreamCheckState, // Live | Fatal { status: u16, cause: String }
    handshake_ms: u64,
}

fn probe_catalog_stream(client: &Client, workspace: &str) -> Option<StreamCheck> {
    // 2s timeout, single short-lived SSE handshake attempt, no retry loop —
    // doctor is a one-shot diagnostic, not a live view.
}
```

### Data Models

No new persisted data model, config schema, or API request/response
type. `DaemonState` is in-memory-only `Model` state, derived on every
status poll — not stored. `StreamCheck` (doctor) is a transient struct for
one CLI invocation's output, never persisted. `batuta doctor --json`'s
`streams` field is additive JSON (see `_dx.md`); no versioning concern
since the whole `--json` payload is unversioned, ad hoc output today.

### API Endpoints

Not applicable — batuta-cli is a client; it defines no API endpoints of
its own. The catalog stream fix touches how an *existing* daemon-defined
endpoint's SSE responses are consumed (`retry_server_errors` is a
client-side retry policy flag, not a new endpoint contract).

## Integration Points

- **CompozyOS daemon** (external system, already integrated): no protocol
  change — `retry_server_errors` and the retry-loop's `probe()` cadence
  are entirely client-side policy. Authentication/error-handling for this
  integration is unchanged from today.
- **CI daemon install** (ADR-003): `go install github.com/compozy/compozy@v0.3.0-beta.16`
  as a new build-time integration point in `.github/workflows/ci.yml`;
  retry/error handling for this step is "fail the job loudly" (US-006
  AC-3) — no silent skip.
- **GitHub Actions release infrastructure** (ADR-004): `peter-evans/create-pull-request`
  for the release-plan PR, a repo-scoped PAT secret for tag creation,
  `cargo-dist`'s generated `release.yml` for build/publish, `orhun/git-cliff-action@v4`
  (`fetch-depth: 0`) for changelog generation.

## Impact Analysis

| Component | Impact Type | Description and Risk | Required Action |
| --- | --- | --- | --- |
| `crates/batuta/src/app.rs` | modified | Adds pre-TUI retry loop; low risk, additive control flow before existing boot path | Implement per ADR-001 |
| `crates/batuta/src/doctor.rs` | modified | Adds `streams` block and enum-backed draining derivation; low risk, additive fields | Implement per ADR-002/`_dx.md` |
| `crates/batuta-tui/src/app/model.rs` | modified | New `DaemonState` enum + field; low risk, additive | Implement per ADR-002 |
| `crates/batuta-tui/src/views/header.rs` | modified | Draining banner replaces ad hoc string compare; low risk | Implement per `_dx.md` |
| `crates/batuta-tui/src/app/update/stream.rs:62-71` | **deleted** | The 503-specific catalog Fatal→10s-poll fallback is removed entirely — root-cause fix (ADR context) makes it dead code, not a compat shim to keep | Delete; covered by US-005 tests |
| `crates/compozy-client/src/sse/catalog.rs:19` | modified | `retry_server_errors: false → true`; behavior change is the fix itself | Implement; add regression test replacing `ut_323_catalog_503_is_fatal`'s now-invalid assertion |
| `.github/workflows/ci.yml` | modified | `contract` job gate removed, install step added; CI time increases (Go build) | Implement per ADR-003 |
| `.github/workflows/release.yml` | **new** | Generated by `cargo dist init`; publishes public binaries | Implement per ADR-004 |
| `dist-workspace.toml`, `cliff.toml` | **new** | Release/changelog tool config | Implement per ADR-004 |
| `LICENSE-MIT`, `LICENSE-APACHE`, `CHANGELOG.md` | **new** | Standard boilerplate + retroactive `git-cliff` generation | Implement per US-010, US-012 |
| `README.md:100-105` | modified | Broken `_dx.md` link replaced with `CONTRIBUTING.md` link | Implement per US-011 |

No fallback/compat shim is kept for the deleted `stream.rs:62-71` special
case — the engine's existing `ReconnectPolicy` fully replaces it.

## Extensibility Integration Plan

Not affected — batuta-cli has no extension manifest, skill, tool, or
bridge surface of its own to change. Checked: `crates/batuta-tui` and
`crates/batuta` expose no `compozy__*` tools, no MCP sidecar, no protocol
docs.

## Agent Manageability Plan

`batuta doctor --json` remains the one machine-readable diagnostic
surface; its new `streams.catalog` field (state, handshake_ms) is the
only new agent-facing structured output this delivery adds. No new CLI
verbs, HTTP endpoints, or UDS routes are introduced — batuta-cli is a
client, not a service, so it exposes no routes of its own.

## Config Lifecycle

No `config.toml` keys added, changed, or removed (Part I decision,
confirmed in `_dx.md`: retry/backoff cadences are fixed, not
operator-configurable, for this delivery). Checked: `crates/batuta`'s
config-loading path (`doctor.rs:117-124` reflects it) requires no
changes.

## Testing Approach

Concrete cases live in `_tests.md`. Strategy:

- **Unit**: `DaemonState::derive` transitions (all three states, boundary
  between Draining/Offline when both conditions could theoretically
  overlap); doctor's `streams` block rendering (human + JSON) for
  Live/Fatal/probe-timeout cases, extending the existing `doctor.rs` test
  module pattern (`ut_060`–`ut_063`, `ut_636`).
- **Integration**: `compozy-client`'s SSE engine tests — replace
  `ut_323_catalog_503_is_fatal` (now describes removed behavior) with an
  equivalent asserting 503 on the catalog stream now retries with
  backoff instead of going Fatal, mirroring the existing tests for
  transcript/loop_events/logs streams that already assert retry
  behavior.
- **E2E / CI**: the `contract` job itself, running against the pinned
  daemon, is the E2E-level proof that CI resilience (US-006) works — no
  separate E2E harness needed beyond what `compozy-testkit` already
  provides. Retry-screen and draining-banner E2E coverage runs through
  `compozy-testkit`'s disposable daemon (start it, kill it mid-run,
  assert the TUI's rendered state — existing harness pattern, new
  scenarios).
- Fakes/mocks sit only at the daemon-connection I/O boundary
  (`compozy-testkit`'s disposable daemon already is that boundary — no
  new mocking layer needed).

## Development Sequencing

### Build Order

1. `compozy-client` catalog fix + `stream.rs` dead-code removal (US-005)
   — smallest, most isolated change; unblocks nothing else but is safest
   to land and verify first (existing SSE test suite gates it).
2. `DaemonState` enum + draining banner (US-003, US-004) — depends on
   nothing from step 1; can run in parallel with it.
3. Pre-TUI retry loop (US-001, US-002) — independent of steps 1–2.
4. `batuta doctor` `streams` block (US-009) — depends on step 1 landing
   first (the probe it performs should reflect the fixed retry behavior,
   not the old Fatal-on-503 path).
5. CI contract job pinning (US-006) — independent of 1–4; can run in
   parallel.
6. Publish-prep (US-010, US-011, US-012 minus the automated-generation
   wiring) — independent, can run anytime.
7. Release pipeline (US-007, US-008) — depends on step 6 (`CHANGELOG.md`
   must exist for `cargo-dist`'s release-notes extraction to have
   something to read on the first real release) and benefits from step 5
   being done first (a release shouldn't ship if contract tests aren't
   actually gating PRs).

Each phase's verification gate is its own test suite passing
(`cargo test`, plus the CI job itself for phases 5 and 7) — no
behavior-changing phase is combined with a cleanup-only phase.

### Technical Dependencies

- A repo-scoped PAT (GitHub secret) must exist before the release
  pipeline (step 7) can be verified end-to-end — an operator-side setup
  action, not something a task can automate (ADR-004 Risks).
- `go` toolchain availability on the CI runner (GitHub-hosted Linux
  runners ship Go; `actions/setup-go` pins the exact version) — no
  external dependency beyond what GitHub Actions already provides.

## Monitoring and Observability

- CI job duration for `contract` (now unconditional) is worth watching
  post-merge — ADR-003's Risk notes it adds Go build time; if it
  regresses PR feedback latency meaningfully, that's a follow-up issue
  (build caching tuning), not a blocker for this delivery.
- No new runtime metrics/log events are introduced in the TUI itself —
  `DaemonState` transitions are UI-only state, not instrumented beyond
  what's already visible on screen. If retry/draining frequency ever
  needs aggregate visibility, that's future scope (not requested here).

## Technical Considerations

### Key Decisions

- Root-cause fix over patch: US-005 is fixed by correcting the
  `retry_server_errors` flag and deleting the resulting dead fallback
  code, not by adding a "re-attempt SSE after N poll cycles" workaround
  on top of the broken flag.
- `DaemonState` lives in `batuta-tui`, not `compozy-client` (ADR-002) —
  keeps the generic/batuta-specific boundary intact even though it costs
  a second small derivation in `doctor.rs`.
- Release automation is assembled from the two named tools plus minimal
  glue, not a third bundled tool, to preserve the issue's literal scope
  (ADR-004).

### Known Risks

- CI time increase from building CompozyOS via `go install` (ADR-003) —
  mitigated by Go build caching; monitor post-merge.
- PAT setup is a manual, easy-to-forget operator step that blocks the
  release pipeline's first real run — flag this prominently in the task
  that implements ADR-004 so it isn't discovered only at release time.
- `git-cliff`'s retroactive changelog generation (US-012) depends on
  every historical commit parsing reasonably under
  `filter_unconventional` — if pre-delivery-1 history is messy, the
  generated `v0.1.0-beta.1` entry may be sparse; accepted per US-012 EC-1.

## Safety Invariants

1. Quitting the pre-TUI retry loop never constructs a `Client` and never
   sends any request to the daemon — no session/loop-run state can be
   touched by a user who quits before a connection exists.
2. The catalog stream's corrected retry behavior must never abandon
   retrying while the daemon is merely draining — only a genuine 4xx
   client error (not 5xx/draining) may still classify as `Fatal`,
   per the existing `ConnectFailure` split in `sse/engine.rs:266-270`,
   which this fix does not alter.
3. `batuta doctor`'s catalog probe (US-009) must never mutate daemon
   state or count as a "connection" for draining write-refusal purposes —
   it is read-only by construction (an SSE handshake, not a write call).
4. The release pipeline must never tag, build, or publish without an
   explicit human merge of the release PR — no code path in the
   `release-plan` job may create a tag directly.

## File References

### Repo Files

- `crates/batuta/src/app.rs:7-19` — current boot path; pre-TUI retry loop
  is added here (ADR-001).
- `crates/batuta/src/main.rs:90-98` — existing `probe()`, reused unchanged
  by the retry loop.
- `crates/batuta-tui/src/app/model.rs:17-22,374-379,419` — `AppMode`,
  `DaemonStatus`, where `DaemonState` is added.
- `crates/batuta-tui/src/views/header.rs:4-13,33-35` — `offline()`
  predicate and current draining string-compare, both feed into
  `DaemonState::derive`.
- `crates/batuta-tui/src/views/{sessions,runs,attention}.rs` — existing
  `Modifier::DIM` pattern, unchanged, still driven by `offline`.
- `crates/batuta-tui/src/app/update/stream.rs:62-71` — the fallback code
  block to delete once `retry_server_errors` is fixed.
- `crates/compozy-client/src/sse/catalog.rs:18-19` — the one-line fix.
- `crates/compozy-client/src/sse/engine.rs:30-61,263-270` —
  `ReconnectPolicy` defaults and `ConnectFailure` classification the fix
  relies on.
- `crates/compozy-client/src/status.rs:11-19` — draining string origin,
  unchanged.
- `crates/compozy-client/tests/engine.rs:118-131` —
  `ut_323_catalog_503_is_fatal`, to be replaced with a retry-not-fatal
  assertion.
- `crates/batuta/src/doctor.rs:8-15,94-127,220-256` — `Report` struct and
  renderers; `streams` block slots in alongside the existing `config`
  field.
- `crates/batuta/src/doctor.rs:279-362` — existing test module pattern
  (`ut_060`–`ut_063`, `ut_636`) to extend.
- `.github/workflows/ci.yml:1-70` — full existing CI structure; `contract`
  job gate (`:63-70`) to change per ADR-003.
- `contract/README.md` — existing daemon-version pin convention (commit
  `a35eda6d` / `v0.3.0-beta.16`), reused for the CI install pin.
- `crates/compozy-testkit/README.md`, root `CLAUDE.md:10-11`,
  `CONTRIBUTING.md:23-24` — the disposable-worktree requirement the CI
  job must keep honoring.
- root `Cargo.toml:9-13` — workspace version/license/edition; version
  field becomes what the release-plan job bumps.
- `README.md:100-105` — the broken link to fix (US-011).
- `CONTRIBUTING.md` — existing dev-workflow content, the link's new
  target.

### Design and Analysis Sources

- `docs/internal/specs/2026-08-17-batuta-cli-design.md:247-257,296-305` —
  original resilience and CI/release intent this delivery implements.
- `adrs/adr-001-startup-retry-pre-tui-loop.md`
- `adrs/adr-002-draining-enum-in-batuta-tui.md`
- `adrs/adr-003-ci-pinned-daemon-via-go-install.md`
- `adrs/adr-004-release-pipeline-git-cliff-cargo-dist.md`

## Assumptions and Defaults

- CI's pinned CompozyOS version starts at `v0.3.0-beta.16`, matching the
  existing OpenAPI contract pin, and is bumped manually in the same PR
  that bumps `contract/*.json` in future deliveries — not auto-synced.
- LICENSE file text is the standard, unmodified MIT and Apache-2.0
  license bodies (the de facto Rust-ecosystem convention referenced in
  market research), not a custom variant.
- The release-plan job's PAT is stored as a repository secret named
  `RELEASE_PLEASE_TOKEN`-equivalent (exact name is an implementation
  detail for the task, not a product decision) — any repo-scoped PAT with
  `contents:write` satisfies the requirement.
- `cargo-dist` targets Linux and macOS only for this delivery's first
  release, per the design doc's existing platform commitment — Windows
  is a Non-Goal.

## Architecture Decision Records

- [ADR-001: Startup retry screen as a pre-TUI loop](adrs/adr-001-startup-retry-pre-tui-loop.md) — retry lives in `app.rs` before `Model`/`Client` construction, not a `Model` restructure.
- [ADR-002: `draining` as a batuta-tui-local enum](adrs/adr-002-draining-enum-in-batuta-tui.md) — `compozy-client` keeps the raw string; `batuta-tui` derives `DaemonState` once.
- [ADR-003: CI installs the pinned daemon via `go install`](adrs/adr-003-ci-pinned-daemon-via-go-install.md) — deterministic version pin, matches the existing contract-pin convention.
- [ADR-004: Hand-assembled release pipeline, no third tool](adrs/adr-004-release-pipeline-git-cliff-cargo-dist.md) — `git-cliff` + `cargo-dist` + minimal PR-gate glue, preserving the issue's literal tool choices.
