---
status: completed
title: "`batuta doctor` stream health"
type: backend
complexity: medium
---

# Task 3: `batuta doctor` stream health

## Overview

Adds a `streams` block to `batuta doctor`'s human and `--json` output,
reporting a short-lived, doctor-owned probe of the workspace catalog
stream — the one stream checkable without a live session — so an operator
can distinguish "daemon is fine, catalog stream itself is unhealthy" from
a plain daemon-down state. Depends on `task_01` because the probe's
result must reflect the corrected (post-fix) catalog retry behavior, not
the removed Fatal-on-503 path.

<critical>
- ALWAYS READ `_spec.md` and its catalogs (`_user_stories.md`, `_dx.md`, `_tests.md`) before starting
- REFERENCE `_spec.md` Part II for implementation details — do not duplicate here
- FOCUS ON "WHAT" — describe what needs to be accomplished, not how
- MINIMIZE CODE — show code only to illustrate current structure or problem areas
- TESTS REQUIRED — implement every test case assigned in ## Tests
</critical>

<requirements>
- MUST perform doctor's own short-lived SSE handshake probe of the catalog endpoint, with a 2-second timeout — MUST NOT attempt to observe another running `batuta` TUI process's state.
- MUST omit session-scoped streams (transcript, loop events, logs) from the `streams` block entirely — MUST NOT report them as `"unknown"` or any placeholder value.
- MUST add the `streams` block additively to both human and `--json` output, matching `_dx.md`'s exact examples (line format, JSON key shape).
- MUST NOT change behavior of any existing doctor check (`ut_060`–`ut_063`, `ut_636`) — this is purely additive.
- MUST keep working when run standalone with no `batuta` TUI session active (doctor's probe is self-contained).
- MUST read draining status through the same enum-backed derivation pattern established in `task_01` rather than a separate raw-string comparison (ADR-002 applies to `doctor.rs` too).
</requirements>

## Subtasks

- [x] 3.1 Implement `probe_catalog_stream` — a 2-second-timeout, single short-lived SSE handshake attempt against the catalog endpoint.
- [x] 3.2 Add the `StreamCheck` result type (`Live { handshake_ms }` / `Fatal { status, cause }` / timeout).
- [x] 3.3 Wire the `streams` block into `doctor::render_human`, matching `_dx.md`'s exact line format.
- [x] 3.4 Wire the `streams` block into `doctor::render_json`, matching `_dx.md`'s exact JSON shape.
- [x] 3.5 Update `doctor.rs`'s existing draining-note logic (`doctor.rs:114-116`) to use the same enum-backed derivation as `task_01`'s `DaemonState`, without duplicating the raw string comparison.
- [x] 3.6 Implement all assigned unit tests (probe outcomes, render formatting, standalone-run regression check).
- [x] 3.7 Implement the assigned E2E journeys (healthy catalog probe, draining catalog probe) against `--json` and human output.

## Implementation Details

Reference `_spec.md` Part II → Implementation Design → Core Interfaces for
the `probe_catalog_stream`/`StreamCheck` shape, and `_dx.md` → CLI →
`batuta doctor` for the exact frozen output format (human and JSON). This
task cannot start meaningfully ahead of `task_01`'s catalog-stream fix
landing, since the probe should exercise the corrected retry-vs-fatal
classification, not the old broken one.

### Relevant Files

- `crates/batuta/src/doctor.rs:8-15,94-127,220-256` — `Report` struct and `render_human`/`render_json`; the `streams` block slots in alongside the existing `config` field, same pattern.
- `crates/batuta/src/doctor.rs:114-116` — existing draining-note logic (`status.daemon.status == "draining"`); update to the enum-backed pattern from `task_01`.
- `crates/batuta/src/doctor.rs:279-362` — existing test module (`ut_060`–`ut_063`, `ut_636`); extend with new cases in the same style, do not restructure existing ones.
- `crates/compozy-client/src/sse/catalog.rs` — the catalog stream endpoint this task's probe connects to (post-`task_01` fix).
- `crates/compozy-client/src/sse/engine.rs:30-61` — `ReconnectPolicy` defaults; doctor's probe is a single attempt, not a retrying client — do not reuse the retry loop, only the connection primitives.

### Dependent Files

- None outside `doctor.rs` — this task's output is additive and self-contained within the `batuta` crate's doctor module.

### Related ADRs

- [ADR-002: `draining` as a batuta-tui-local enum](adrs/adr-002-draining-enum-in-batuta-tui.md) — applies to `doctor.rs`'s independent derivation too (it cannot import `batuta-tui`'s enum without inverting the crate graph; see ADR-002's "Alternatives Considered").

## Deliverables

- `streams` block in `batuta doctor` human and `--json` output, matching `_dx.md` exactly.
- Draining-note logic in `doctor.rs` uses the same enum-backed pattern as `task_01`, not a duplicated raw-string comparison.
- Every test case assigned in `## Tests` implemented and passing **(REQUIRED)**.

## Tests

Cases assigned from `_tests.md` — read each ID's full definition there before writing tests.

- [x] UT-011 — `probe_catalog_stream` against healthy daemon returns `Live` within 2s
- [x] UT-012 — `probe_catalog_stream` against draining daemon returns `Fatal { 503, "daemon draining" }`
- [x] UT-013 — `probe_catalog_stream` timeout boundary at exactly 2s
- [x] UT-014 — `render_human` streams line format matches `_dx.md`
- [x] UT-015 — `render_json` streams.catalog shape matches `_dx.md`
- [x] UT-016 — existing doctor checks unaffected when no TUI session is running
- [x] E2E-004 — `batuta doctor` human output shows live catalog probe
- [x] E2E-005 — `batuta doctor --json` shows fatal catalog probe during draining

## Success Criteria

- Every assigned test case implemented and passing.
- `batuta doctor` and `batuta doctor --json` output match `_dx.md`'s examples byte-for-byte in structure (values will vary, e.g. timing).
- All pre-existing doctor tests (`ut_060`–`ut_063`, `ut_636`) still pass unmodified in behavior.
