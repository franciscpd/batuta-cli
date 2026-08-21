---
status: pending
title: Lossless transcript presentation and adaptive layout
type: frontend
complexity: high
---

# Lossless transcript presentation and adaptive layout

## Overview

Deliver the conversation-first presentation layer: complete messages, reversible operational disclosure, raw debug fidelity, stable live-follow behavior, and responsive panel allocation. This task changes derived TUI state and rendering while preserving source data and pure views.

<critical>
- ALWAYS READ `_spec.md` and its catalogs (`_user_stories.md`, `_dx.md`, `_uiux.md` when present, `_tests.md`) before starting
- REFERENCE `_spec.md` Part II for implementation details — do not duplicate here
- FOCUS ON "WHAT" — describe what needs to be accomplished, not how
- MINIMIZE CODE — show code only to illustrate current structure or problem areas
- TESTS REQUIRED — implement every test case assigned in ## Tests
</critical>

<requirements>
- Derived transcript rows MUST retain stable source references and preserve eligible source order, count, and values when expanded.
- Normal mode MUST show complete conversational content and safely collapse operational detail; debug mode MUST expose ordered raw source data without loss.
- Wrapping, selection, follow-tail, and unseen-update behavior MUST remain stable across streaming updates and resize.
- Layout MUST apply the documented narrow, medium, and wide relevance policy without unreachable or invisible content panels.
- Views MUST render from `Model` only and MUST NOT perform I/O or import `compozy_client`.
</requirements>

## Visual Contract

| ID | Reference artifact + state | Implementation target + state | Viewport | Fidelity | Authorized differences + authority |
| --- | --- | --- | --- | --- | --- |
| VC-01 | `_uiux.md` — conversation/activity rows | Transcript normal mode | 120x40 | normative | Existing Ratatui glyph geometry may vary only when content and hierarchy remain intact |
| VC-02 | `_uiux.md` — collapsed and expanded tool | Transcript disclosure | 100x30 | normative | None |
| VC-03 | `_uiux.md` — global debug mode | Raw transcript mode | 120x40 | normative | Pagination is allowed without truncation per `_spec.md` |
| VC-04 | `_uiux.md` — narrow layout | One-content-panel layout | 90x30 | normative | None |
| VC-05 | `_uiux.md` — medium layout | Detail plus contextual panel | 120x40 | normative | None |
| VC-06 | `_uiux.md` — wide layout | Relevance-weighted multi-panel layout | 180x50 | normative | None |

Evidence for each row: `.compozy/tasks/ux-foundation/evidence/visual/task_02/<contract-id>/{reference.png,implementation.png,side-by-side.png,diff.png,comparison.json,review.md}`.

## Subtasks

- [ ] Define derived transcript rows, grouping boundaries, expansion, and raw-debug presentation state.
- [ ] Render full conversational messages and human-first operational/error disclosure.
- [ ] Implement Unicode-safe wrapping, source anchoring, follow-tail, and unseen-update behavior.
- [ ] Implement deterministic relevance scoring and narrow, medium, and wide layout allocation.
- [ ] Invalidate presentation cache on every specified source, mode, width, and expansion input.
- [ ] Add the required render matrix, integration coverage, and end-to-end journeys.

## Implementation Details

Build derived presentation from raw model state and preserve the one-way data flow in `_spec.md` Part II. Keep `crates/batuta-tui/src/views/` pure.

### Relevant Files

- `crates/batuta-tui/src/transcript.rs` — transcript state and source handling.
- `crates/batuta-tui/src/render_cache.rs` — presentation cache behavior.
- `crates/batuta-tui/src/app/model.rs` — model-owned derived state and relevance.
- `crates/batuta-tui/src/app/update/stream.rs` — transcript update path.
- `crates/batuta-tui/src/views/transcript.rs` — transcript rendering.
- `crates/batuta-tui/src/views/layout.rs` — pure adaptive layout policy.
- `crates/batuta-tui/tests/tail_mode.rs` — follow-tail test patterns.
- `crates/batuta-tui/tests/layout.rs` — layout test patterns.

### Dependent Files

- `crates/batuta-tui/src/views/mod.rs` — view composition.
- `crates/batuta-tui/src/keymap.rs` and `crates/batuta-tui/src/app/update/keys.rs` — transcript controls.
- `crates/batuta-tui/tests/render.rs` and `crates/batuta-tui/tests/snapshots/` — render matrix coverage.

## Deliverables

- Lossless derived transcript presentation with normal and debug modes.
- Stable wrapping and follow-tail behavior.
- Deterministic responsive layout policy.
- Every Visual Contract row has a durable passing evidence bundle.
- Every test case assigned in `## Tests` implemented and passing **(REQUIRED)**.

## Tests

- [ ] UT-706, UT-707, UT-708 — full messages, disclosure, and human-first errors.
- [ ] UT-709, UT-710, UT-711, UT-712, UT-713 — reversible grouping and raw debug fidelity.
- [ ] UT-714, UT-715, UT-716, UT-717, UT-718 — wrapping, anchors, unseen updates, and follow-tail.
- [ ] UT-720, UT-721, UT-722, UT-723, UT-724 — adaptive layout and relevance policy.
- [ ] UT-732 — all 54 canonical render snapshots and their required assertions.
- [ ] IT-700, IT-701, IT-702 — real transcript update, follow-tail, and resize paths.
- [ ] E2E-701, E2E-702, E2E-703 — conversation/debug, long live transcript, and adaptive resize journeys.

## Success Criteria

- Every assigned test case implemented and passing.
- Expanded groups reconstruct eligible source data exactly in order and value.
- All 54 canonical render snapshots pass.
- Every Visual Contract row is `PASS` with zero unresolved blocking divergence.
