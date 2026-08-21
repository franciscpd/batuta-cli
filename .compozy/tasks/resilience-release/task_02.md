---
status: completed
title: Startup retry screen
type: backend
complexity: medium
---

# Task 2: Startup retry screen

## Overview

Replaces `batuta`'s current behavior of printing an error and exiting
when the daemon is unreachable at launch with a self-recovering retry
screen: a small pre-TUI loop that owns the terminal, retries the
connection every 3 seconds, shows the specific failure cause, and
transitions automatically into the normal session view once the daemon
answers — or exits cleanly if the operator quits.

<critical>
- ALWAYS READ `_spec.md` and its catalogs (`_user_stories.md`, `_dx.md`, `_tests.md`) before starting
- REFERENCE `_spec.md` Part II for implementation details — do not duplicate here
- FOCUS ON "WHAT" — describe what needs to be accomplished, not how
- MINIMIZE CODE — show code only to illustrate current structure or problem areas
- TESTS REQUIRED — implement every test case assigned in ## Tests
</critical>

<requirements>
- MUST NOT restructure `Model`, `Cmd`, or `RuntimeClient` to support a disconnected state — the retry screen is a pre-TUI loop in `crates/batuta/src/app.rs`, per ADR-001.
- MUST retry on a flat 3-second cadence, indefinitely, with no automatic give-up.
- MUST show the specific last-probe error (e.g. connection refused vs. no socket file vs. version mismatch), never a generic "can't connect" message.
- MUST transition automatically into the existing boot path (construct `Client`, `Model`, call `runtime::run`) the moment `probe()` succeeds — no operator action required.
- MUST allow the operator to quit from the retry screen at any time via the existing quit keybind, exiting with code 0 and never constructing a `Client`.
- MUST NOT start, cancel, or otherwise touch any session or loop run on the daemon from the retry screen or on quit from it (project-wide invariant, `CLAUDE.md`).
- MUST reuse the existing `probe()` function (`crates/batuta/src/main.rs`) rather than duplicating probing logic.
</requirements>

## Subtasks

- [x] 2.1 Implement the pre-TUI retry loop in `crates/batuta/src/app.rs`, owning the same terminal setup already used for the main run.
- [x] 2.2 Implement the retry screen's static render (attempt counter, specific last-error text) using the existing `ratatui` setup.
- [x] 2.3 Wire the quit keybind to exit the loop cleanly with code 0.
- [x] 2.4 Wire successful `probe()` to fall through into the existing boot path unchanged.
- [x] 2.5 Handle the version-mismatch probe outcome distinctly from a plain connection-refused outcome in the rendered text.
- [x] 2.6 Implement all assigned unit tests (retry-screen rendering, quit path).
- [x] 2.7 Implement all assigned integration tests using `compozy-testkit`'s disposable daemon (delayed start, never-appears socket, flap-during-transition, quit/connect race).
- [x] 2.8 Implement the assigned E2E journeys (daemon-down-then-up, quit-from-retry-screen), matching `_dx.md`'s Golden Path transcripts exactly.

## Implementation Details

Reference `_spec.md` Part II → Implementation Design → Core Interfaces for
the `await_daemon` function shape and ADR-001 for why this is a pre-TUI
loop rather than a `Model` restructure. Match `_dx.md`'s Golden Path
section for the exact retry-screen text and transitions — the DX contract
is frozen; do not improvise different wording.

### Relevant Files

- `crates/batuta/src/app.rs:7-19` — current boot path; the retry loop is added here, before the existing `Client`/`Model` construction.
- `crates/batuta/src/main.rs:90-98` — existing `probe()` (3s timeout), reused unchanged by the retry loop.
- `crates/batuta-tui/src/runtime/mod.rs:458` — the existing `runtime::run(model, client, &mut terminal)` call the loop falls through into on success; confirm its terminal-mode assumptions are compatible with the retry loop's own terminal ownership.
- `crates/batuta/src/version.rs` — `MIN_COMPOZY_VERSION`, used by `doctor.rs:212,251`; the retry screen's version-mismatch case should reuse the same version-floor check, not a new one.

### Dependent Files

- `crates/batuta/src/main.rs` — the entry point that currently calls `probe()` once and exits on failure; this call site changes to invoke the new retry loop instead.

### Related ADRs

- [ADR-001: Startup retry screen as a pre-TUI loop](adrs/adr-001-startup-retry-pre-tui-loop.md) — the architectural boundary this task implements.

## Deliverables

- Pre-TUI retry loop in `crates/batuta/src/app.rs` replacing the current print-and-exit behavior.
- Retry screen matching `_dx.md`'s Golden Path exactly (attempt counter, specific error, quit hint).
- Every test case assigned in `## Tests` implemented and passing **(REQUIRED)**.

## Tests

Cases assigned from `_tests.md` — read each ID's full definition there before writing tests.

- [x] UT-017, UT-018 — retry screen render (attempt counter, specific error text)
- [x] UT-019 — quit path returns exit code 0, no `Client` ever constructed
- [x] UT-020 — version-mismatch text distinct from connection-refused text
- [x] IT-001 — daemon starts late, TUI transitions automatically without restart
- [x] IT-002 — socket never appears, retries forever with consistent specific error
- [x] IT-003 — daemon flaps during transition, no panic or partial session view
- [x] IT-004 — quit/connect race resolves cleanly
- [x] E2E-001 — full daemon-down-then-up journey matching `_dx.md` Golden Path
- [x] E2E-002 — quit from retry screen, exit code 0, nothing created

## Success Criteria

- Every assigned test case implemented and passing.
- `batuta` launched against an unreachable daemon opens the TUI on the retry screen instead of printing to stderr and exiting.
- No changes to `Model`, `Cmd`, or `RuntimeClient` signatures (verified by diff — this task's scope is additive-only in `app.rs`).
