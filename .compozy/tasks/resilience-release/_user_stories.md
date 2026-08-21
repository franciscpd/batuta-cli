# User Stories: Resilience & Release (Delivery 3)

Canonical behavior catalog for batuta-cli's third delivery: daemon-absence
and draining resilience, automated contract testing, the release pipeline,
`batuta doctor` reinforcement, and publish-prep. Companion to `_spec.md`;
consumed by `_spec.md` Part II (component mapping) and `_tests.md`
(coverage matrix). No UI surfaces outside the existing TUI are introduced,
so there is no `_uiux.md` for this delivery — resilience UX changes are
covered here and mapped directly in `_spec.md` Part II.

## Personas

- **TUI Operator** — runs `batuta` day to day against a local or remote
  CompozyOS daemon; needs the TUI to stay honest and usable when the
  daemon is slow, draining, or briefly unreachable, without losing work
  or misreading stale state as fresh.
- **Contributor** — opens pull requests against batuta-cli and, once
  trusted, cuts releases; needs CI to catch daemon-contract regressions
  automatically and needs the release process to be safe to trigger
  without hand-crafting version bumps or changelogs.
- **New Adopter** — discovers batuta-cli on GitHub, downloads a release
  binary, and runs it for the first time; needs clear licensing, a
  trustworthy changelog, working documentation links, and a diagnostic
  command that tells them what is wrong when setup fails.

## Story Index

| ID     | Feature Area          | Persona       | Story                                                        |
| ------ | ---------------------- | ------------- | ------------------------------------------------------------- |
| US-001 | Startup Resilience     | TUI Operator  | See a retry screen instead of a crash when the daemon is down |
| US-002 | Startup Resilience     | TUI Operator  | Exit the retry screen manually at any time                    |
| US-003 | Draining UX            | TUI Operator  | See a distinct draining indicator, not a generic offline one  |
| US-004 | Draining UX            | TUI Operator  | Keep reading state while writes are refused during draining   |
| US-005 | Stream Recovery        | TUI Operator  | Have the catalog stream self-heal after the daemon un-drains  |
| US-006 | CI Contract Tests      | Contributor   | Get contract-test feedback on every PR automatically          |
| US-007 | Release Pipeline       | Contributor   | Cut a release by merging a reviewed version-bump PR            |
| US-008 | Release Pipeline       | New Adopter   | Download a checksummed binary from a GitHub Release            |
| US-009 | Doctor Reinforcement   | TUI Operator  | Diagnose stream health with `batuta doctor`                    |
| US-010 | Publish-Prep           | New Adopter   | Read the project's dual-license terms                          |
| US-011 | Publish-Prep           | Contributor   | Follow a working README link to contributor docs               |
| US-012 | Publish-Prep           | New Adopter   | Read what changed in each released version                     |

## Startup Resilience

### US-001: See a retry screen instead of a crash when the daemon is down

**As a** TUI Operator, **I want** `batuta` to show a retry screen when it
can't reach the daemon at startup, **so that** I don't have to manually
re-run the command every few seconds while the daemon finishes starting.

Acceptance criteria:

- AC-1: Given the daemon is unreachable, when I run `batuta`, then the TUI
  opens showing a retry screen instead of printing an error and exiting.
- AC-2: Given the retry screen is showing, when the daemon becomes
  reachable, then the TUI transitions automatically into the normal
  session view without requiring a restart.
- AC-3: Given the retry screen is showing, then it displays the last
  connection error (e.g., no socket file vs. connection refused vs. no
  response) rather than a generic "can't connect" message.

Edge cases:

- EC-1: Daemon socket file never appears (misconfigured `COMPOZY_HOME`) →
  retry screen keeps retrying indefinitely and keeps showing the specific
  error; it never silently gives up.
- EC-2: Daemon appears, then disappears again before the transition
  finishes → TUI stays on/returns to the retry screen, no partial or
  crashed session view.
- EC-3: Daemon responds but with an incompatible/too-old schema version →
  retry screen shows the version-mismatch error distinctly from a
  connection error, per the existing `version::check` floor.

### US-002: Exit the retry screen manually at any time

**As a** TUI Operator, **I want** to quit out of the retry screen whenever
I choose, **so that** I'm not stuck waiting if I decide not to start the
daemon after all.

Acceptance criteria:

- AC-1: Given the retry screen is showing, when I press the quit keybind,
  then `batuta` exits immediately with a clean, non-panic exit code.
- AC-2: Given I quit from the retry screen, then no session or loop run is
  started or affected — quitting the TUI never touches daemon-side state,
  consistent with the existing quit invariant.

Edge cases:

- EC-1: Quit pressed the instant the daemon becomes reachable (race
  between transition and quit) → quit wins cleanly, no dangling connection
  or half-initialized session view left behind.

## Draining UX

### US-003: See a distinct draining indicator, not a generic offline one

**As a** TUI Operator, **I want** a draining daemon to look visibly
different from an unreachable one, **so that** I know the daemon is
shutting down gracefully rather than assuming something crashed.

Acceptance criteria:

- AC-1: Given the daemon reports `draining` status, when I look at the
  header/status area, then it shows a draining-specific banner distinct
  from the existing "daemon offline" dimmed state.
- AC-2: Given the daemon transitions from `draining` back to a healthy
  status (aborted drain), then the banner clears and the UI returns to
  normal without a restart.

Edge cases:

- EC-1: Daemon goes from `draining` straight to unreachable (drain
  completes, process exits) → banner transitions from draining to
  offline/retry, not stuck showing draining forever.
- EC-2: Draining status flaps rapidly (daemon reports draining, then
  healthy, then draining again within seconds) → banner reflects the
  latest known state each time, no stale banner left from a superseded
  poll response.

### US-004: Keep reading state while writes are refused during draining

**As a** TUI Operator, **I want** to keep viewing sessions, runs, and logs
while the daemon drains, **so that** I don't lose visibility into
in-flight work just because new actions are blocked.

Acceptance criteria:

- AC-1: Given the daemon is draining, when I browse existing sessions,
  runs, or transcripts, then read views keep working normally.
- AC-2: Given the daemon is draining, when I attempt a write action
  (create session, send a prompt, start a run), then the action is
  refused with a draining-specific message, not a generic error.

Edge cases:

- EC-1: A write action is in flight when draining begins → the in-flight
  request either completes or fails with a clear draining-caused message;
  it never hangs silently.
- EC-2: Draining daemon with zero active sessions (first-run / empty
  state) → read views show their normal empty state, not an error, since
  there is nothing in-flight to lose visibility into.

## Stream Recovery

### US-005: Have the catalog stream self-heal after the daemon un-drains

**As a** TUI Operator, **I want** the workspace/session catalog to resume
live updates after the daemon finishes draining, **so that** I don't have
to restart `batuta` just because the daemon briefly cycled.

Acceptance criteria:

- AC-1: Given the catalog stream fell back to polling because the daemon
  returned 503/draining, when the daemon becomes healthy again, then the
  catalog stream re-establishes SSE instead of remaining on polling
  indefinitely.
- AC-2: Given the catalog stream has recovered to SSE, then catalog
  updates (new/removed sessions or workspaces) arrive with the same
  latency as before the degradation, not the slower polling cadence.

Edge cases:

- EC-1: Daemon flaps between draining and healthy multiple times while
  polling is active → stream does not attempt to re-establish SSE on
  every single flap in a tight loop; it recovers without hammering the
  daemon (bounded retry cadence).
- EC-2: Workspace switch happens while the catalog stream is degraded and
  polling → existing workspace-switch recovery path and the new
  self-heal path do not conflict or double-start the stream.

## CI Contract Tests

### US-006: Get contract-test feedback on every PR automatically

**As a** Contributor, **I want** contract tests to run against a real
CompozyOS daemon on every pull request, **so that** I find out about
daemon-contract regressions before merge, not after.

Acceptance criteria:

- AC-1: Given I open a PR that touches `compozy-client`, when CI runs,
  then the `contract` job runs automatically against a pinned, installed
  CompozyOS daemon binary — no `workflow_dispatch` or manual env var
  required.
- AC-2: Given the contract job fails, then the PR check clearly reports
  which contract test(s) failed, same as any other CI job.
- AC-3: Given the pinned daemon binary can't be installed (download
  failure, version not found), then the job fails loudly with that cause,
  never silently skipping and reporting green.

Edge cases:

- EC-1: PR does not touch `compozy-client` or contract-relevant code →
  job still runs (simplicity over selective triggering) but this is
  recorded as an accepted cost, not a bug, unless the spec's Part II
  decides otherwise.
- EC-2: Contract test run from a checkout containing `.compozy/` (CI
  runner state leak) → job must still honor the existing "disposable
  detached worktree" requirement; this is a real risk to design against
  in CI, not just local dev.

## Release Pipeline

### US-007: Cut a release by merging a reviewed version-bump PR

**As a** Contributor (with release authority), **I want** a bot-maintained
"release PR" with the version bump and changelog diff, **so that** I can
review exactly what's about to ship before anything is tagged or
published.

Acceptance criteria:

- AC-1: Given commits land on `main` since the last release, when the
  release automation runs, then it opens or updates a standing release PR
  showing the next version and the generated changelog diff.
- AC-2: Given I merge the release PR, then a version tag is created and
  `release.yml` builds and publishes binaries for the supported platforms
  with checksums, without further manual steps.
- AC-3: Given the release PR is never merged, then no tag, build, or
  publish happens — the human gate is real, not cosmetic.

Edge cases:

- EC-1: Two feature PRs merge to `main` before the release PR is merged →
  the release PR updates to include both, it does not create duplicate
  release PRs.
- EC-2: A commit does not follow Conventional Commits format → it is
  excluded from the generated changelog section (per `filter_unconventional`)
  rather than corrupting the changelog output.
- EC-3: Release PR merge and a fresh Contributor push race → tag creation
  uses a token that reliably triggers the downstream `release.yml`
  workflow (not the default `GITHUB_TOKEN`, which does not trigger
  tag-push workflows).

### US-008: Download a checksummed binary from a GitHub Release

**As a** New Adopter, **I want** to download a prebuilt `batuta` binary
with a verifiable checksum, **so that** I can install it without building
from source or trusting an unverified download.

Acceptance criteria:

- AC-1: Given a release is published, when I open its GitHub Release page,
  then I see platform binaries (at minimum Linux and macOS, per the
  design doc's stated targets) with accompanying checksum files.
- AC-2: Given I download a binary and its checksum, when I verify it,
  then the checksum matches — build reproducibility is not broken by the
  pipeline.

Edge cases:

- EC-1: A platform build fails during the release workflow → the release
  either fails as a whole (no partial/inconsistent release) or clearly
  marks which platform is missing, never silently omitting a binary.

## Doctor Reinforcement

### US-009: Diagnose stream health with `batuta doctor`

**As a** TUI Operator (or a Contributor helping debug someone else's
setup), **I want** `batuta doctor` to report on SSE stream health, not
just the one-shot daemon status, **so that** I can tell whether a
"nothing updates" complaint is a stream stuck in degraded polling versus
an actually-healthy daemon.

Acceptance criteria:

- AC-1: Given `batuta doctor` runs while a `batuta` TUI session's streams
  are in a degraded state (Reconnecting/Fatal/stuck-polling), when doctor
  has visibility into that state, then it surfaces it in both human and
  `--json` output.
- AC-2: Given no TUI session is running (doctor run standalone), then
  doctor's existing one-shot checks continue to work exactly as before —
  stream-health reporting is additive, not a regression on the plain
  invocation.

Edge cases:

- EC-1: Doctor run standalone with no way to observe another process's
  live stream state → this is recorded as an Open Question for Part II
  (doctor is a separate process from the TUI; whether stream-health
  reporting is TUI-internal diagnostics vs. a true `doctor` feature needs
  a technical answer, not a product one).

## Publish-Prep

### US-010: Read the project's dual-license terms

**As a** New Adopter, **I want** `LICENSE-MIT` and `LICENSE-APACHE` files
in the repo root, **so that** I can confirm the terms under which I may
use, modify, or redistribute batuta-cli before I rely on it.

Acceptance criteria:

- AC-1: Given I open the repo root on GitHub, then `LICENSE-MIT` and
  `LICENSE-APACHE` are both present with standard, unmodified license
  text.
- AC-2: Given `Cargo.toml` declares `license = "MIT OR Apache-2.0"`, then
  both license files exist and match that declaration exactly — no drift
  between the manifest and the actual files.

Edge cases:

- EC-1: A workspace member crate has its own `Cargo.toml` `license` field
  → it is consistent with the root declaration, not silently different.

### US-011: Follow a working README link to contributor docs

**As a** Contributor, **I want** the README's "Contributing" section to
link to a document that actually exists and renders on GitHub, **so
that** I'm not dropped on a 404 when I try to learn how to contribute.

Acceptance criteria:

- AC-1: Given I open `README.md` on GitHub, when I click the
  "Contributing" section's dev-docs link, then it resolves to a tracked,
  non-git-ignored file that renders content (e.g. `CONTRIBUTING.md`),
  not `.compozy/tasks/mvp-tui/_dx.md`.
- AC-2: Given the linked document, then it covers what a new contributor
  needs (toolchain, pre-PR checks, crate-boundary rules) — the intent
  behind the original `_dx.md` link is preserved, not just removed.

Edge cases:

- EC-1: Future deliveries add new dev-workflow docs under `docs/` → this
  fix does not preclude linking additional docs later; it just stops
  linking a file that cannot exist on GitHub.

### US-012: Read what changed in each released version

**As a** New Adopter, **I want** a `CHANGELOG.md` covering every released
version including the ones before this delivery, **so that** I can judge
what's new or changed before upgrading, and understand what v0.1.0-beta.1
already contained.

Acceptance criteria:

- AC-1: Given I open `CHANGELOG.md`, then it has an entry for
  `v0.1.0-beta.1` reconstructed from historical commits, not just entries
  starting from this delivery onward.
- AC-2: Given a new version is released after this delivery, when
  `git-cliff` runs as part of the release pipeline, then a new entry is
  appended automatically — no manual changelog editing required per
  release.

Edge cases:

- EC-1: Historical commits before Conventional Commits discipline was
  adopted (if any) don't parse cleanly → they are excluded via
  `filter_unconventional` rather than producing malformed changelog
  entries; this is recorded as an accepted gap in historical coverage,
  not a bug to chase.
