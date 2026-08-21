---
status: completed
title: Generic workspace registration and deterministic resolution
type: backend
complexity: medium
---

# Generic workspace registration and deterministic resolution

## Overview

Deliver generic daemon-client registration support plus deterministic workspace selection before the TUI starts. This creates the contract consumed by onboarding while keeping `compozy-client` independent of Ratatui and Batuta-specific names.

<critical>
- ALWAYS READ `_spec.md` and its catalogs (`_user_stories.md`, `_dx.md`, `_uiux.md` when present, `_tests.md`) before starting
- REFERENCE `_spec.md` Part II for implementation details — do not duplicate here
- FOCUS ON "WHAT" — describe what needs to be accomplished, not how
- MINIMIZE CODE — show code only to illustrate current structure or problem areas
- TESTS REQUIRED — implement every test case assigned in ## Tests
</critical>

<requirements>
- Resolution MUST honor non-empty `--workspace`, then non-empty `COMPOZY_WORKSPACE`, then canonical longest-prefix cwd matching.
- Invalid or ambiguous explicit selectors MUST identify their source and value and MUST NOT fall back to cwd selection.
- An unmatched canonical cwd MUST produce an unresolved candidate without any write.
- The generic client MUST expose workspace registration with a typed `Unsupported` result for unavailable daemon support.
- `crates/compozy-client` MUST NOT depend on Ratatui or contain Batuta-specific names.
</requirements>

## Subtasks

- [x] Define generic workspace-add request, response, and unsupported outcome types.
- [x] Implement the daemon registration request and typed endpoint/capability handling.
- [x] Implement explicit selector and canonical longest-prefix workspace resolution.
- [x] Represent unresolved canonical directories as candidates suitable for later onboarding.
- [x] Add boundary, transport, symlink, and end-to-end resolution coverage.

## Implementation Details

Follow the workspace resolution algorithm and generic client contract in `_spec.md` Part II. This task must expose a stable interface for `task_04` without implementing its UI state machine.

### Relevant Files

- `crates/compozy-client/src/workspaces.rs` — current workspace client surface.
- `crates/compozy-client/src/types/workspace.rs` — generic workspace types.
- `crates/compozy-client/src/request.rs` — request and error-envelope behavior.
- `crates/compozy-client/tests/writes.rs` — request/write test patterns.
- `crates/batuta/src/workspace.rs` — workspace selection and resolution boundary.

### Dependent Files

- `crates/compozy-client/src/lib.rs` — generic client exports.
- `crates/batuta/src/main.rs` — startup integration, if applicable.
- `crates/batuta-tui/src/app/model.rs` — consumes the resolved selection/candidate contract in task_04.

## Deliverables

- Generic registration client API and typed unsupported outcome.
- Deterministic, canonical workspace resolver with unresolved candidate support.
- Boundary checks that preserve client and view separation.
- Every test case assigned in `## Tests` implemented and passing **(REQUIRED)**.

## Tests

- [x] UT-725, UT-726, UT-727, UT-728 — precedence, explicit-error, canonical-path, and candidate behavior.
- [x] UT-730 — generic-client and pure-view boundary check.
- [x] IT-704, IT-705 — generic add request and unsupported response mapping.
- [x] E2E-704 — workspace resolution journey.

## Success Criteria

- Every assigned test case implemented and passing.
- The generic client has no Ratatui dependency and no Batuta-specific names.
- Explicit invalid selectors never silently select a cwd workspace.
