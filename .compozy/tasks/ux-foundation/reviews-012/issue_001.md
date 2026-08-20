---
round: 12
round_created_at: 2026-08-20T23:25:14.730597943Z
status: valid
file: crates/batuta-tui/src/transcript.rs
line: 171
severity: high
author: unknown
---

# Issue 001: Failed tool events are collapsed as routine telemetry

## Review Comment

`operational_kind` groups any tool whose `error_text` is absent, including `state: "output-error"`. But `views::cards::tool_status` explicitly maps that state to `ActivityState::Failed`. Therefore two adjacent failed tool updates without a textual error payload collapse into one muted group, hiding the required visible failed state until Enter is pressed. This violates the UX contract that errors must never be absorbed into collapsed groups. Exclude error/failure states (including `output-error`) from grouping and add a regression test for adjacent failed tools without `error_text`.

## Triage

- Decision: `VALID`
- Root cause: `operational_kind` accepted every tool entry without `error_text`,
  including the canonical `output-error` state. The renderer maps that state to
  `ActivityState::Failed`, so adjacent failed entries could be hidden in a muted
  presentation group.
- Fix: exclude canonical failed tool states from operational grouping and cover
  two adjacent `output-error` entries without `error_text`.
