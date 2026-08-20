---
round: 9
round_created_at: 2026-08-20T22:32:53.038543281Z
status: valid
file: crates/batuta-tui/src/views/layout.rs
line: 76
severity: high
author: unknown
---

# Issue 002: Medium and wide layouts ignore panel relevance and compaction

## Review Comment

For every width of100 columns or more, layout allocation is fixed at a40% left rail, three fixed40/30/30 panel slices, and a60% detail pane. It never reads panel population, attention, active work, or focus. Consequently empty panels retain substantial permanent space and a pending-attention/active panel cannot grow, contrary to the required medium contextual rail and wide relevance-weighted allocation. The focused layout tests only cover compact mode, so this regression remains untested despite `cargo test -p batuta-tui --test render --test layout --test picker` passing.

## Triage

- Decision: `VALID`
- Root cause: the full-layout branch used fixed 40/30/30 vertical percentages
  for all contextual panels, independently of the panel state in `Model`.
- Fix: rank contextual panels by the specified relevance order, allocate the
  remaining rail height only to the highest-ranked non-empty/relevant panel,
  and leave peers as compact three-row status panels. Add medium and wide
  layout tests for active work, pending attention, tie order, empty compaction,
  and the transcript width guarantee.
