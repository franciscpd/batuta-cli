---
status: completed
title: Semantic terminal theme and configuration
type: frontend
complexity: medium
---

# Semantic terminal theme and configuration

## Overview

Deliver the semantic, terminal-safe visual language for the TUI. It must preserve terminal-default prose while making every operational state understandable with or without color.

<critical>
- ALWAYS READ `_spec.md` and its catalogs (`_user_stories.md`, `_dx.md`, `_uiux.md` when present, `_tests.md`) before starting
- REFERENCE `_spec.md` Part II for implementation details — do not duplicate here
- FOCUS ON "WHAT" — describe what needs to be accomplished, not how
- MINIMIZE CODE — show code only to illustrate current structure or problem areas
- TESTS REQUIRED — implement every test case assigned in ## Tests
</critical>

<requirements>
- The TUI MUST map semantic intent to ANSI-16-safe styles while normal text inherits terminal foreground and background.
- `ui.theme = auto|dark|light` MUST be validated with the exact documented error for invalid values.
- `NO_COLOR` MUST take precedence over theme and color configuration without changing content, ordering, or layout.
- Every state MUST retain a textual label, glyph, or focus treatment when color is absent.
</requirements>

## Visual Contract

| ID | Reference artifact + state | Implementation target + state | Viewport | Fidelity | Authorized differences + authority |
| --- | --- | --- | --- | --- | --- |
| VC-01 | `_uiux.md` — dark semantic tokens | TUI semantic theme — dark | 120x40 | normative | Terminal default prose remains unstyled per `_uiux.md` |
| VC-02 | `_uiux.md` — light semantic tokens | TUI semantic theme — light | 120x40 | normative | Terminal default prose remains unstyled per `_uiux.md` |
| VC-03 | `_uiux.md` — no-color state markers | TUI with `NO_COLOR` | 120x40 | normative | No color attributes; labels and glyphs remain |

Evidence for each row: `.compozy/tasks/ux-foundation/evidence/visual/task_01/<contract-id>/{reference.png,implementation.png,side-by-side.png,diff.png,comparison.json,review.md}`.

## Subtasks

- [x] Define semantic tokens and dark, light, auto, and no-color resolution.
- [x] Apply semantic styles to the shared TUI rendering surface.
- [x] Extend configuration parsing and diagnostics for `ui.theme`.
- [x] Preserve the existing `ui.color` and `NO_COLOR` precedence contract.
- [x] Add unit, render, and fixture-daemon coverage for the semantic states.

## Implementation Details

Extend the existing theme and settings patterns; keep rendering presentation-only and retain terminal compatibility constraints from `_spec.md` Part II.

### Relevant Files

- `crates/batuta-tui/src/theme.rs` — existing theme implementation.
- `crates/batuta-tui/src/app/model.rs` — UI settings and model state.
- `crates/batuta-tui/src/views/` — consumers of semantic styles.
- `crates/batuta-tui/tests/render.rs` — rendering test conventions.

### Dependent Files

- `crates/batuta-tui/tests/snapshots/` — semantic render evidence.
- `crates/batuta-tui/src/lib.rs` — settings/configuration integration if needed.

## Deliverables

- Semantic theme implementation with documented precedence behavior.
- Validated `ui.theme` configuration surface.
- Every Visual Contract row has a durable passing evidence bundle.
- Every test case assigned in `## Tests` implemented and passing **(REQUIRED)**.

## Tests

- [x] UT-700, UT-701, UT-702 — default prose, ANSI-16 mappings, and non-color state distinctions.
- [x] UT-703, UT-704, UT-705 — theme configuration, `NO_COLOR`, and automatic variant resolution.
- [x] UT-719 — activity-state labels, markers, and tokens including no-color mode.
- [x] E2E-700 — semantic terminal journey.

## Success Criteria

- Every assigned test case implemented and passing.
- Normal prose inherits terminal defaults in dark and light variants.
- Every Visual Contract row is `PASS` with zero unresolved blocking divergence.
