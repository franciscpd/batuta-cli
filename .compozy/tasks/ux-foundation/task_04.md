---
status: pending
title: Confirmed workspace onboarding in the TUI
type: frontend
complexity: high
---

# Confirmed workspace onboarding in the TUI

## Overview

Deliver the TUI state machine for unresolved directories: confirmation, registration, refetch, selection, boot, picker, and actionable recovery. It consumes the generic client and resolver contract from `task_03` and never starts processes or mutates workspace state without explicit confirmation.

<critical>
- ALWAYS READ `_spec.md` and its catalogs (`_user_stories.md`, `_dx.md`, `_uiux.md` when present, `_tests.md`) before starting
- REFERENCE `_spec.md` Part II for implementation details — do not duplicate here
- FOCUS ON "WHAT" — describe what needs to be accomplished, not how
- MINIMIZE CODE — show code only to illustrate current structure or problem areas
- TESTS REQUIRED — implement every test case assigned in ## Tests
</critical>

<requirements>
- The model MUST represent resolving, selected, unregistered, confirmation, adding, unsupported, and failed workspace-startup states.
- Opening, cancelling, choosing, or exiting onboarding MUST emit no registration write; only confirmed add may emit exactly one add request.
- A successful add MUST refetch, select by canonical root, and complete normal boot before onboarding closes.
- Unsupported and error states MUST be human-first, actionable, and retain refresh, choose, and exit paths without hidden retry.
- Quitting the TUI MUST NOT stop or cancel a session or loop run.
</requirements>

## Visual Contract

| ID | Reference artifact + state | Implementation target + state | Viewport | Fidelity | Authorized differences + authority |
| --- | --- | --- | --- | --- | --- |
| VC-01 | `_uiux.md` — unregistered directory | Onboarding candidate | 100x30 | normative | None |
| VC-02 | `_uiux.md` — add confirmation | Confirmation dialog | 100x30 | normative | None |
| VC-03 | `_uiux.md` — progress and unsupported | Adding and unsupported states | 100x30 | normative | Fallback command follows `_dx.md` exactly |
| VC-04 | `_uiux.md` — failure recovery | Human-first error state | 100x30 | normative | Diagnostic details may be collapsed |

Evidence for each row: `.compozy/tasks/ux-foundation/evidence/visual/task_04/<contract-id>/{reference.png,implementation.png,side-by-side.png,diff.png,comparison.json,review.md}`.

## Subtasks

- [ ] Add workspace-startup state and messages to the TUI model.
- [ ] Implement confirmation, picker, exit, refresh, and explicit add transitions.
- [ ] Integrate generic registration outcomes with catalog refetch, canonical selection, and boot sequencing.
- [ ] Render candidate, confirmation, progress, unsupported, and human-first failure states from `Model` only.
- [ ] Preserve no-mutation behavior across cancellation and recovery paths.
- [ ] Add model, transport-capture, render, and end-to-end coverage.

## Implementation Details

Implement the onboarding state machine in `batuta-tui` using the generic interfaces introduced by `task_03`; keep client I/O outside `src/views/`.

### Relevant Files

- `crates/batuta-tui/src/app/model.rs` — application state.
- `crates/batuta-tui/src/app/update/api.rs` — async effect results.
- `crates/batuta-tui/src/app/update/picker.rs` — picker transitions.
- `crates/batuta-tui/src/views/picker.rs` — picker rendering.
- `crates/batuta-tui/src/views/overlays.rs` — confirmation and error presentation.
- `crates/batuta-tui/tests/picker.rs` — picker test patterns.
- `crates/batuta-tui/tests/update.rs` — model/update test patterns.

### Dependent Files

- `crates/batuta/src/workspace.rs` — resolution contract producer.
- `crates/compozy-client/src/workspaces.rs` — registration contract provider.
- `crates/batuta-tui/src/cmd.rs` — generic effects/commands.
- `crates/batuta-tui/tests/snapshots/` — onboarding render evidence.

## Deliverables

- Workspace onboarding model, update flow, and pure views.
- Explicitly confirmed registration and recoverable result states.
- Every Visual Contract row has a durable passing evidence bundle.
- Every test case assigned in `## Tests` implemented and passing **(REQUIRED)**.

## Tests

- [ ] UT-729 — zero-write cancellation, picker, and exit transitions.
- [ ] UT-731 — unsupported fallback, no subprocess, and human-first generic errors.
- [ ] IT-703 — no-write onboarding paths.
- [ ] IT-706, IT-707, IT-708, IT-709, IT-710, IT-711 — success, conflict, error, indeterminate, post-add failure, and recovery behavior.
- [ ] E2E-705, E2E-706, E2E-707 — confirmed registration, unsupported/error, and no-mutation journeys.

## Success Criteria

- Every assigned test case implemented and passing.
- Exactly one confirmed add request occurs per confirmation, and no other route writes.
- Onboarding closes only after successful refetch, selection, and boot.
- Every Visual Contract row is `PASS` with zero unresolved blocking divergence.
