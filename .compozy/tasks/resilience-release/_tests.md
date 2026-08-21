# Test Specification: Resilience & Release (Delivery 3)

Canonical test contract for batuta-cli's third delivery. Companion to
`_spec.md`. Derived from `_user_stories.md` (behavior), `_spec.md` Part II
(components), and `_dx.md` (CLI journeys — this delivery has no browser
surface, so no `_uiux.md`/browser E2E cases exist).

## Strategy

- **Frameworks**: Rust's built-in `#[test]`/`#[tokio::test]` for unit and
  integration tests (matching existing `ut_NNN`-style naming already used
  in `doctor.rs` and `compozy-client/tests/`), `compozy-testkit`'s
  disposable daemon for integration/E2E cases that need a real daemon
  process. GitHub Actions workflow files are tested by their own runs
  (IT-013–IT-021, E2E-006–E2E-007 are verified by observing real CI/release
  runs, not a local harness — there is no offline simulator for GitHub
  Actions in this repo).
- **Execution**: unit/integration tests run via `cargo test` (workspace-
  wide, same as today); contract/CI-workflow cases run via the actual
  `ci.yml`/`release.yml` on a real PR/tag, per this project's existing
  `compozy-testkit` convention (disposable detached worktree, no
  `.compozy/` in the checkout).
- **Conventions**: unit tests are table-driven where multiple `DaemonState`
  transitions share a shape; fakes sit only at the daemon-connection I/O
  boundary (the disposable daemon process itself, never a hand-rolled
  mock of the SSE protocol).

## Coverage Matrix

| Source | Behavior | Unit | Integration | E2E |
| --- | --- | --- | --- | --- |
| US-001 | Retry screen on daemon-absent startup | UT-017, UT-018 | IT-001 | E2E-001 |
| US-001.EC-1 | Socket never appears, retries forever | — | IT-002 | — |
| US-001.EC-2 | Daemon flaps during transition | — | IT-003 | — |
| US-001.EC-3 | Version-mismatch shown distinctly | UT-020 | — | — |
| US-002 | Manual exit from retry screen | UT-019 | — | E2E-002 |
| US-002.EC-1 | Quit/connect race | — | IT-004 | — |
| US-003 | Distinct draining banner | UT-005, UT-006 | — | E2E-003 |
| US-003.EC-1 | Draining → unreachable transition | — | IT-006 | — |
| US-003.EC-2 | Draining flaps rapidly | — | IT-005 | — |
| US-004 | Reads work, writes refused while draining | UT-007, UT-008 | — | E2E-003 |
| US-004.EC-1 | In-flight write when draining begins | — | IT-007 | — |
| US-004.EC-2 | Draining with zero active sessions | — | IT-008 | — |
| US-005 | Catalog stream self-heals | UT-009, UT-010 | IT-009, IT-010 | — |
| US-005.EC-1 | Flapping doesn't storm-reconnect | — | IT-011 | — |
| US-005.EC-2 | Workspace switch vs. self-heal race | — | IT-012 | — |
| US-006 | Contract tests run on every PR | — | IT-013 | E2E-006 |
| US-006.EC-1 | Job runs even on unrelated PRs (accepted) | — | IT-014 | — |
| US-006.EC-2 | Worktree/`.compozy/` requirement honored | — | IT-015 | — |
| US-007 | Reviewed release PR gate | — | IT-016, IT-019, IT-020 | E2E-007 |
| US-007.EC-1 | No duplicate release PRs | — | IT-017 | — |
| US-007.EC-2 | Non-conventional commit excluded | — | IT-018 | — |
| US-007.EC-3 | PAT triggers downstream tag-push workflow | — | IT-019 | — |
| US-008 | Checksummed binaries on GitHub Releases | — | — | E2E-007, E2E-008 |
| US-008.EC-1 | Platform build failure handled, not hidden | — | IT-021 | — |
| PR CI hardening | Boundary check works without a warm Cargo cache | — | IT-022 | — |
| PR CI hardening | Retry-screen PTY output and cleanup are deterministic | — | IT-023 | — |
| US-009 | `batuta doctor` reports stream health | UT-011–UT-016 | — | E2E-004, E2E-005 |
| US-009.EC-1 | Catalog endpoint itself unhealthy | UT-012 | — | E2E-005 |
| US-010 | LICENSE files match `Cargo.toml` declaration | — | — | E2E-009 |
| US-010.EC-1 | Workspace-member `license` field consistency | UT-021 | — | — |
| US-011 | Working README Contributing link | — | — | E2E-009 |
| US-011.EC-1 | Future docs additions don't conflict | — | — | (design-only, no test — see note) |
| US-012 | Full-history CHANGELOG | — | — | E2E-009 |
| US-012.EC-1 | Pre-convention commits excluded, not malformed | — | IT-018 | — |
| `DaemonState::derive` | Part II Core Interface | UT-001–UT-004 | — | — |
| `probe_catalog_stream` | Part II Core Interface | UT-011–UT-013 | — | — |
| Deleted `stream.rs:62-71` fallback | Impact Analysis | UT-010 | — | — |

US-011.EC-1 ("future docs additions don't preclude linking more later") is
a non-testable forward-compatibility note, not an observable behavior —
recorded here as intentionally uncovered rather than left silently
missing.

## Unit Tests

### `DaemonState::derive` (Spec: Implementation Design → Core Interfaces)

- **UT-001** (happy): `DaemonState::derive` — given `status.status ==
  "draining"`, `offline = false`, returns `DaemonState::Draining`.
- **UT-002** (happy): `DaemonState::derive` — given `status.status ==
  "ok"`, `offline = true`, returns `DaemonState::Offline`.
- **UT-003** (happy): `DaemonState::derive` — given `status.status ==
  "ok"`, `offline = false`, returns `DaemonState::Connected`.
- **UT-004** (boundary): `DaemonState::derive` — given `status.status ==
  "draining"` AND `offline = true` simultaneously, returns
  `DaemonState::Draining` (draining takes precedence — a draining daemon
  is never mis-rendered as a hard offline).

### Header/View rendering (Spec: System Architecture → `batuta-tui`)

- **UT-005** (happy): header render with `DaemonState::Draining` — output
  contains the draining-specific banner text and does not contain the
  offline-dimmed banner text.
- **UT-006** (state): header render transitioning `Draining → Connected`
  across two successive `Model` updates — second render shows no
  draining banner.
- **UT-007** (error): write-action dispatch (`create_session`) with
  `DaemonState::Draining` — returns the draining-specific refusal message
  from `_dx.md`'s Errors table, not a generic error.
- **UT-008** (happy): sessions/runs list render with `DaemonState::Draining`
  — output is identical to a `Connected` render of the same underlying
  session data (reads unaffected).

### Catalog stream fix (Spec: Implementation Design, Impact Analysis)

- **UT-009** (happy): `compozy-client` catalog SSE request builder — after
  the fix, `retry_server_errors == true` on the constructed request.
- **UT-010** (boundary): `batuta-tui`'s stream-update handler — a 503
  `ConnectFailure` for `StreamId::Catalog` no longer matches any
  special-cased branch (the deleted `stream.rs:62-71` block); falls
  through to the same generic retry-status handling as other stream IDs.

### `batuta doctor` streams block (Spec: Implementation Design → Core Interfaces)

- **UT-011** (happy): `probe_catalog_stream` against a healthy daemon —
  returns `StreamCheck { state: Live, handshake_ms: <2000 }` within the
  2s timeout.
- **UT-012** (error): `probe_catalog_stream` against a daemon returning
  503 (draining) on the catalog endpoint — returns `StreamCheck { state:
  Fatal { status: 503, cause: "daemon draining" } }`.
- **UT-013** (boundary): `probe_catalog_stream` against a daemon that
  never responds — returns a timeout result at exactly the 2s boundary,
  not hanging past it.
- **UT-014** (happy): `doctor::render_human` with a `Live` `StreamCheck` —
  output line matches `_dx.md`'s `streams     catalog: live (handshake
  42ms)` format exactly.
- **UT-015** (happy): `doctor::render_json` with a `Live` `StreamCheck` —
  JSON `streams.catalog` object matches `_dx.md`'s example shape
  (`state`, `handshake_ms` keys).
- **UT-016** (happy): `doctor::run()` with no `batuta` TUI process
  running — all pre-existing checks (`ut_060`–`ut_063`, `ut_636`) still
  pass unchanged; `streams` block is present regardless (doctor performs
  its own probe, independent of any other process).

### Startup retry screen (Spec: Implementation Design → Core Interfaces)

- **UT-017** (happy): `render_retry_screen` called with `attempt = 4` —
  output contains "attempt 4".
- **UT-018** (happy): `render_retry_screen` called with a connection-
  refused probe error — output contains the specific cause text ("last
  error: connection refused"), not a generic message.
- **UT-019** (happy): `await_daemon` loop receiving a quit signal before
  any successful `probe()` — returns `AppError::reported(0)`; no `Client`
  value is ever constructed (asserted by construction — the function
  cannot return `Ok` on the quit path).
- **UT-020** (error): `render_retry_screen` given a probe error carrying a
  schema-version mismatch — output text is distinct from (does not equal)
  the connection-refused text.

### Publish-prep consistency (Spec: Impact Analysis)

- **UT-021** (happy): a check (script or test) asserting every workspace
  member `Cargo.toml`'s `license` field equals the root's
  `"MIT OR Apache-2.0"` — passes for all four crates after the fix.

## Integration Tests

### Startup retry (US-001, US-002)

- **IT-001**: start `batuta` against `compozy-testkit`'s disposable daemon
  spawned 6 seconds late — assert the retry screen renders for the gap,
  then the TUI transitions to the normal session view within one retry
  cycle after the daemon becomes reachable, no restart.
- **IT-002**: start `batuta` with `COMPOZY_HOME` pointing at a socket path
  that never gets created — assert the retry screen keeps retrying past
  5 cycles, each render showing the same specific "no socket file" cause.
- **IT-003**: start the disposable daemon, let `batuta` begin its
  transition, kill the daemon before the transition completes — assert
  `batuta` returns to (or remains on) the retry screen, no panic, no
  partially-initialized session view.
- **IT-004**: race a quit keypress against the daemon becoming reachable
  (both triggered in the same tick) — assert the process exits cleanly
  with no session view ever rendered and no daemon connection left open.

### Draining UX (US-003, US-004)

- **IT-005**: disposable daemon toggled `draining → ok → draining` three
  times within 2 seconds — assert the header banner matches the latest
  toggle at each poll, never showing a state older than the last known
  status.
- **IT-006**: disposable daemon transitions `draining` then process exits
  — assert the banner moves from the draining banner to the
  offline/retry state, never stuck showing draining.
- **IT-007**: issue a prompt-send write request, then flip the daemon to
  draining before the response arrives — assert the request either
  completes or fails with the draining-specific message within a bounded
  timeout (no indefinite hang).
- **IT-008**: disposable daemon with zero sessions, flipped to draining —
  assert the sessions/runs views render their existing empty state, not
  an error state.

### Catalog stream self-heal (US-005)

- **IT-009**: disposable daemon serving a 503 on the catalog endpoint for
  3 seconds then recovering — assert the catalog stream reconnects via
  SSE (not stuck on polling) once the daemon recovers, replacing the now-
  invalid `ut_323_catalog_503_is_fatal` assertion.
- **IT-010**: after IT-009's recovery, create a new session on the
  disposable daemon — assert the catalog stream reflects it within SSE-
  latency bounds (not the old 10s polling cadence).
- **IT-011**: disposable daemon flapping draining/healthy 5 times within
  10 seconds — assert the client's reconnect attempts stay within the
  existing `ReconnectPolicy` bounds (0.5s–10s backoff), no tight
  reconnect loop.
- **IT-012**: trigger a workspace switch while the catalog stream is
  mid-backoff from a 503 — assert exactly one catalog stream ends up
  running afterward, not two.

### CI contract job (US-006)

- **IT-013**: a PR branch touching `crates/compozy-client` triggers
  `ci.yml`'s `contract` job automatically (no `workflow_dispatch`, no
  `COMPOZY_TEST_DAEMON_BIN` set by the invoker) — job installs the pinned
  daemon via `go install` and the contract tests run to completion.
- **IT-014**: a PR branch touching only `README.md` still triggers the
  `contract` job (accepted cost per US-006.EC-1, not selectively skipped).
- **IT-015**: the `contract` job's checkout is confirmed to contain no
  `.compozy/` directory before tests run (fresh Actions checkout
  satisfies this by construction — asserted as a guard step, not just
  assumed).

### Release pipeline (US-007, US-008)

- **IT-016**: two commits land on `main` — the `release-plan` job opens a
  PR titled `release: vX.Y.Z` containing the `Cargo.toml` version bump and
  the `git-cliff`-generated changelog diff for those two commits.
- **IT-017**: a third commit lands on `main` before the release PR from
  IT-016 is merged — assert the same PR updates (new commit included),
  no second release PR is opened.
- **IT-018**: a commit not following Conventional Commits format lands
  among the others — assert it is absent from the generated changelog
  section (`filter_unconventional = true`), and no other entries are
  corrupted by it.
- **IT-019**: merge the release PR — assert a tag is created using the
  configured PAT (not `GITHUB_TOKEN`) and that the tag push visibly
  triggers `release.yml`.
- **IT-020**: open a release PR and do not merge it — assert no tag, no
  `release.yml` run, and no published artifact exists afterward.
- **IT-021**: simulate (or observe on a real run) one platform's
  `cargo-dist` build step failing — assert the release workflow fails as
  a whole rather than publishing a Release with a silently missing
  platform binary.

### PR CI hardening

- **IT-022**: run `scripts/check-boundaries.sh` with an empty temporary
  Cargo home and the committed lockfile — assert dependency metadata is
  resolved and every existing boundary passes; then introduce a
  representative forbidden crate dependency in an isolated disposable
  checkout and assert the same script fails for that boundary.
- **IT-023**: launch the retry-screen E2E process through its PTY harness
  with the daemon socket permanently absent — assert fixed terminal
  dimensions and flushed capture expose the attempt counter and specific
  last-probe error, then exercise normal quit and forced-timeout cleanup
  and assert both paths reap the child within the configured deadline.

## End-to-End Tests

### Startup and draining journeys (US-001, US-002, US-003, US-004)

- **E2E-001**: launch `batuta` with the daemon down → retry screen with
  live attempt counter and specific error → start the daemon → TUI
  transitions to the normal session view automatically, matching
  `_dx.md`'s Golden Path transcript.
- **E2E-002**: launch `batuta` with the daemon down → press `q` on the
  retry screen → process exits with code `0`, no session or run exists
  anywhere afterward.
- **E2E-003**: live `batuta` session against a running daemon → trigger
  draining → header shows the draining banner → browse sessions/runs/logs
  successfully → attempt to start a new session → see the exact refusal
  toast from `_dx.md` → daemon finishes draining and recovers → banner
  clears without restarting `batuta`.

### Doctor journeys (US-009)

- **E2E-004**: run `batuta doctor` against a healthy daemon — human
  output includes `streams     catalog: live (handshake <N>ms)` matching
  `_dx.md`'s example format.
- **E2E-005**: run `batuta doctor --json` against a draining daemon —
  JSON output's `streams.catalog.state == "fatal"` with a `cause`
  mentioning draining, matching `_dx.md`'s example.

### CI and release journeys (US-006, US-007, US-008)

- **E2E-006**: open a real PR on the repo touching `compozy-client` — the
  GitHub PR checks UI shows the `contract` job running and completing
  automatically, no manual trigger by any human.
- **E2E-007**: full release journey on the real repo — merge a release
  PR → tag appears → GitHub Release is published with Linux and macOS
  binaries plus checksum files → release notes match the `CHANGELOG.md`
  section for that version.
- **E2E-008**: download a published binary and its checksum file from a
  GitHub Release → verify locally (`sha256sum -c`) → checksum matches.

### Publish-prep journey (US-010, US-011, US-012)

- **E2E-009**: open the repo root on GitHub (or a local clone rendered as
  GitHub would) — `LICENSE-MIT` and `LICENSE-APACHE` are present and
  render as plain text; `README.md`'s Contributing section link resolves
  to `CONTRIBUTING.md` and renders (not a 404); `CHANGELOG.md` shows a
  `v0.1.0-beta.1` entry alongside any later versions.
