---
status: completed
title: Retry-screen PTY harness reliability
type: bugfix
complexity: medium
---

# Task 8: Retry-screen PTY harness reliability

## Overview

Stabilizes the retry-screen integration harness across local and GitHub Actions
PTY environments. The harness must expose the rendered attempt/error text and
must always terminate or forcibly reap its child within a bounded time.

<critical>
- ALWAYS READ `_spec.md` and its catalogs (`_user_stories.md`, `_dx.md`, `_tests.md`) before starting
- PRESERVE the production retry cadence, copy, quit behavior, and pre-TUI architecture
- FIX the test/process boundary rather than weakening assertions or adding sleeps blindly
- TESTS REQUIRED — implement every test case assigned in ## Tests
</critical>

<requirements>
- MUST make the retry-screen PTY dimensions deterministic and flush rendered output so CI can observe the specific last-error text and attempt counter.
- MUST keep IT-002's semantic assertion: the missing socket retries repeatedly with the same specific error.
- MUST bound process shutdown; on timeout the harness must terminate and reap the entire spawned PTY process path without hanging the test suite.
- MUST preserve exit-code-zero behavior when the application handles the quit key normally.
- MUST NOT change production retry timing or replace the PTY integration test with a render-only unit test.
- MUST avoid leaking child processes after success, failure, or timeout.
</requirements>

## Subtasks

- [x] 8.1 Reproduce the CI symptom using the existing retry-screen integration test.
- [x] 8.2 Align the retry-screen PTY setup with the stable dimensions and flushing used by the existing live TUI harness.
- [x] 8.3 Replace unbounded shutdown waiting with bounded terminate-and-reap cleanup.
- [x] 8.4 Add regression coverage for visible retry output and bounded cleanup.
- [x] 8.5 Run the affected E2E test under the stable and MSRV toolchains used by CI where applicable.

## Implementation Details

The real PR run passed the other PTY cases but `it_002_missing_socket_retries_with_the_same_specific_error`
captured only alternate-screen control bytes. Earlier local attempts also
showed that `quit_retry_screen` can block indefinitely in `wait_with_output`.
Treat these as one harness ownership problem: deterministic PTY presentation
plus bounded lifecycle cleanup.

### Relevant Files

- `crates/batuta/tests/e2e.rs` — retry-screen process helper, quit helper, and IT-002 regression.
- `crates/batuta/src/app.rs` — production retry-screen behavior whose contract must remain unchanged.
- `.github/workflows/ci.yml` — stable and MSRV commands that exercise the E2E suite.

### Dependent Files

- None.

## Deliverables

- Reliable retry-screen output capture on GitHub Actions and locally.
- Bounded child-process cleanup with no leaked PTY process.
- IT-002 and the new harness regression pass without weakening their assertions.

## Tests

Cases assigned from `_tests.md` — read each ID's full definition there before implementation.

- [x] IT-023 — retry-screen PTY exposes the specific retry text and always terminates within the cleanup bound

Existing regression that MUST remain passing (ownership remains with `task_02`):

- IT-002 — socket never appears, retries forever with consistent specific error

## Success Criteria

- IT-002 passes on GitHub Actions with the expected specific last-error text visible.
- Every helper exit path reaps its child within a fixed deadline.
- Normal quit still exits with code 0.
- No production retry semantics or architecture are changed.
