---
status: completed
title: Draining state & stream resilience
type: backend
complexity: high
---

# Task 1: Draining state & stream resilience

## Overview

Consolidates draining detection into a single `DaemonState` enum consumed
by the header banner and write-action guards, and fixes the root cause of
the catalog stream getting permanently stuck on 10s polling after a
503/draining response by correcting `retry_server_errors` and deleting
the dead fallback code it made unreachable. This is the foundational
resilience slice other tasks (`task_03`) build on.

<critical>
- ALWAYS READ `_spec.md` and its catalogs (`_user_stories.md`, `_dx.md`, `_tests.md`) before starting
- REFERENCE `_spec.md` Part II for implementation details — do not duplicate here
- FOCUS ON "WHAT" — describe what needs to be accomplished, not how
- MINIMIZE CODE — show code only to illustrate current structure or problem areas
- TESTS REQUIRED — implement every test case assigned in ## Tests
</critical>

<requirements>
- MUST add a `DaemonState { Connected, Draining, Offline }` enum in `batuta-tui`'s `Model`, derived in exactly one place from `DaemonStatus.status` and the existing `offline()` predicate, per ADR-002.
- MUST NOT add any batuta-specific type to `compozy-client` — `status.rs` keeps returning the raw `"draining"` string unchanged.
- MUST make the draining banner in the header visually and textually distinct from the existing offline-dimmed state.
- MUST keep all read views (sessions, runs, attention, logs, transcripts) fully functional while `DaemonState::Draining`.
- MUST refuse write actions (session create, prompt send, run start, and any other write) while `DaemonState::Draining`, surfacing a draining-specific message per `_dx.md`'s Errors table.
- MUST set `retry_server_errors = true` on the catalog SSE stream request in `crates/compozy-client/src/sse/catalog.rs`.
- MUST delete the now-dead 503-specific Fatal→10s-poll fallback in `crates/batuta-tui/src/app/update/stream.rs` (no fallback/compat shim kept — Impact Analysis in `_spec.md`).
- MUST replace `ut_323_catalog_503_is_fatal` (`crates/compozy-client/tests/engine.rs`) with an equivalent asserting the catalog stream now retries with backoff on 503 instead of going Fatal.
- MUST NOT introduce a new `TimerId` for catalog recovery — the existing `ReconnectPolicy` exponential backoff (already used by transcript/loop_events/logs streams) covers it once the flag is fixed.
</requirements>

## Subtasks

- [x] 1.1 Add `DaemonState` enum and its single derivation function to `batuta-tui`'s `Model`.
- [x] 1.2 Wire `DaemonState` into the header render, replacing the ad hoc `model.daemon.status == "draining"` string compare.
- [x] 1.3 Add the draining banner to `header.rs`, visually distinct from the existing offline-dimmed style.
- [x] 1.4 Wire write-action dispatch paths (session create, prompt send, run control, approve/reject, cancel turn, task verbs, clarification answers) to check `DaemonState::Draining` and refuse with the `_dx.md` message; confirmed read views are unaffected.
- [x] 1.5 Flip `retry_server_errors` to `true` in `crates/compozy-client/src/sse/catalog.rs`.
- [x] 1.6 Delete the dead 503-specific fallback block in `crates/batuta-tui/src/app/update/stream.rs`; the generic retry-status handling now covers catalog 503s. Also removed the now-fully-dead `catalog_polling` field and `TimerId::CatalogPoll` timer (nothing could set them anymore — keeping them would have been exactly the "fallback/compat shim" the Impact Analysis forbids).
- [x] 1.7 Replaced `ut_323_catalog_503_is_fatal` with `ut_323_catalog_503_retries_with_backoff`; also fixed `crates/batuta-tui/tests/panels_sessions.rs`'s `ut_473_catalog_503_falls_back_to_polling`, which asserted the same now-deleted behavior at the `batuta-tui` update-handler level.
- [x] 1.8 Implemented all assigned unit tests: UT-001–UT-010 (`crates/batuta-tui/tests/daemon_state.rs`, `crates/compozy-client/src/sse/catalog.rs` inline test, `crates/compozy-client/tests/engine.rs`).
- [x] 1.9 Implemented IT-005..IT-012. Catalog recovery cases run against `compozy-testkit`'s real disposable daemon through deterministic catalog-only 503 injection; TUI state/race cases exercise the production update/render boundary. Daemon-backed execution was verified from a disposable detached worktree with a temporary `COMPOZY_HOME`.
- [x] 1.10 Implemented E2E-003 as the full production update/render journey: draining banner, session/run/log reads, exact write refusal, and banner clear on recovery.

Subtasks 1.1–1.10 now close the actual defect and its complete assigned
regression contract: the catalog stream no longer gets stuck after a 503,
`DaemonState` drives the header/write guards, and the disposable-daemon plus
update/render journeys cover degradation, recovery, flapping, and switching.

## Implementation Details

Reference `_spec.md` Part II → Implementation Design → Core Interfaces for
the `DaemonState` shape, and Impact Analysis for the exact deletion
target. Follow ADR-002 for the crate-boundary rationale (why the enum
lives in `batuta-tui`, not `compozy-client`) and the Safety Invariant #2
in `_spec.md` (catalog retry must never abandon retrying while merely
draining — only genuine 4xx errors may still classify `Fatal`).

### Relevant Files

- `crates/batuta-tui/src/app/model.rs:17-22,374-379,419` — `AppMode`, `DaemonStatus`; add `DaemonState` and its field here.
- `crates/batuta-tui/src/views/header.rs:4-13,33-35` — `offline()` predicate and current draining string-compare; both feed `DaemonState::derive`, and this file gets the new banner.
- `crates/batuta-tui/src/views/sessions.rs:97-99`, `runs.rs:113-115`, `attention.rs:78-80` — existing `Modifier::DIM` dimming pattern; confirm unaffected by draining (reads must keep working).
- `crates/batuta-tui/src/app/update/api.rs:52,132-136,305` — existing `unavailable()` classification and draining toast wording (`"daemon is draining"`, `"daemon is draining — writes refused"`); extend this pattern to every write action, not just session-create/prompt.
- `crates/batuta-tui/src/app/update/stream.rs:62-71` — the fallback block to delete.
- `crates/compozy-client/src/sse/catalog.rs:18-19` — the one-line fix.
- `crates/compozy-client/src/sse/engine.rs:30-61,263-270` — `ReconnectPolicy` defaults and `ConnectFailure` classification the fix relies on; do not alter this logic, only the flag catalog.rs passes into it.
- `crates/compozy-client/src/status.rs:11-19` — draining string origin; confirm unchanged.
- `crates/compozy-client/tests/engine.rs:108-131` — `ut_322_catalog_event_ignores_ready_comment` (neighboring test, pattern to mirror) and `ut_323_catalog_503_is_fatal` (to replace).
- `crates/compozy-client/tests/engine.rs:259-270` — `ut_336_client_errors_are_fatal_server_errors_retry`, the existing pattern for asserting retry-vs-fatal classification; mirror its style for the new catalog assertion.
- `crates/batuta-tui/src/app/update/timers.rs:41-47` — existing `TimerId::StatusPoll`/`TimerId::CatalogPoll` wiring; confirm no new timer is added, only the removal of the polling-fallback code path.

### Dependent Files

- `crates/batuta-tui/src/views/mod.rs:45-53` — passes `offline: bool` into view renders; may need to also pass or derive `DaemonState` depending on how the banner is composed.

### Related ADRs

- [ADR-002: `draining` as a batuta-tui-local enum](adrs/adr-002-draining-enum-in-batuta-tui.md) — the crate-boundary decision this task implements.

## Deliverables

- `DaemonState` enum in `batuta-tui`, single derivation point, consumed by header and write-guard call sites.
- Draining banner distinct from the offline-dimmed state.
- Catalog stream self-heals after a 503/draining response; dead fallback code removed.
- Every test case assigned in `## Tests` implemented and passing **(REQUIRED)**.

## Tests

Cases assigned from `_tests.md` — read each ID's full definition there before writing tests.

- [x] UT-001, UT-002, UT-003, UT-004 — `DaemonState::derive` transitions and precedence
- [x] UT-005, UT-006 — header banner render and clear-on-recovery
- [x] UT-007, UT-008 — write-refusal while draining, reads unaffected
- [x] UT-009 — catalog SSE request builder has `retry_server_errors == true`
- [x] UT-010 — deleted fallback block no longer intercepts catalog 503s
- [x] IT-005 — draining flaps rapidly, banner always reflects latest state
- [x] IT-006 — draining → unreachable transition, banner never stuck
- [x] IT-007 — in-flight write when draining begins, completes or fails clearly
- [x] IT-008 — draining with zero active sessions, normal empty state
- [x] IT-009 — catalog stream reconnects via SSE after 503 recovery (replaces `ut_323_catalog_503_is_fatal`)
- [x] IT-010 — recovered catalog stream delivers updates at SSE latency, not polling cadence
- [x] IT-011 — flapping daemon doesn't cause a reconnect storm (bounded `ReconnectPolicy`)
- [x] IT-012 — workspace switch during degraded catalog stream doesn't double-start it
- [x] E2E-003 — full draining journey: banner, reads work, write refused, recovery, banner clears

## Success Criteria

- Every assigned test case implemented and passing.
- `cargo test -p batuta-tui -p compozy-client` passes with no reintroduced flakiness in the SSE engine suite.
- No raw `"draining"` string comparison remains outside the single `DaemonState::derive` function within `batuta-tui`.
- `stream.rs:62-71`'s original fallback block no longer exists in the codebase (verified by diff, not just by test passing).
