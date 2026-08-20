---
round: 5
round_created_at: 2026-08-20T21:40:36.890234085Z
status: valid
file: crates/batuta-tui/tests/render.rs
line: 143
severity: high
author: unknown
---

# Issue 001: Required54-case render matrix is missing

## Review Comment

The canonical contract requires the full3 themes ×3 widths ×6 fixtures snapshot matrix, including ANSI-16 and NO_COLOR style assertions (`_tests.md:24-45`), and task_02 assigns it as required UT-732 plus its associated integration/E2E coverage (`task_02.md:81-89`). `crates/batuta-tui/tests/render.rs:143-146` creates only three snapshots of one fixture at80x24,120x40, and200x60; its sole semantic journey checks one120x40 model. Repository search also finds no UT-706–724 implementations. Thus empty/long/tool/error/attention states, the required90x30 and180x50 layouts, and their dark/light/NO_COLOR combinations have no regression proof. Add the parameterized54-row matrix and the specified style/layout assertions, then implement the assigned transcript/layout integration and E2E cases.

## Triage

- Decision: `VALID`
- Notes: The contract and local code confirm the missing matrix: the prior test
  sampled one transcript fixture at three sizes, with no empty, short, long,
  error, or attention fixture and no 90x30/180x50 coverage. The renderer also
  exposed raw tool-input scalars while collapsed, contrary to the required
  progressive-disclosure boundary. The remediation adds the 54-row matrix with
  ANSI-16/NO_COLOR assertions and focused tool/error disclosure coverage, and
  removes raw input summaries from collapsed tool rows. The existing
  `render` integration path (`Msg::Resize` → render-cache rebuild → Ratatui
  view) is the canonical suite for these render invariants.
