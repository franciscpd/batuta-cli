---
round: 10
round_created_at: 2026-08-20T22:49:33.079242672Z
status: valid
file: crates/batuta-tui/src/views/layout.rs
line: 88
severity: high
author: unknown
---

# Issue 003: Wide layout gives all surplus rail height to one panel

## Review Comment

The full-layout branch uses the same single `grown` panel and fixed three-line constraints at every width >=100. At180 columns, even when multiple contextual panels are populated/relevant, only the highest-ranked panel receives surplus space and every other panel remains compact. Wide mode is required to distribute the contextual remainder by relevance, while the one-grown-panel rule applies only to medium mode (`_spec.md:125-128,360-365`; `_tests.md:142-147`). Split medium and wide allocation policies and add a180x50 multi-panel relevance case.

## Triage

- Decision: `VALID`
- Root cause: the full-layout branch applied `contextual_panel_to_grow` and
  fixed three-line peer constraints at both medium and wide widths, so wide
  layouts could not distribute height among multiple relevant panels.
- Fix: retain the single grown-panel policy below 140 columns, while wide
  layouts use relevance-weighted fill constraints for every non-empty
  contextual panel and compact only zero-relevance panels. The 180x50
  regression test covers populated Sessions, active Runs, and pending
  Attention receiving progressively more height.
