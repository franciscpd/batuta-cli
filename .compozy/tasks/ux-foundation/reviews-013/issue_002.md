---
round: 13
round_created_at: 2026-08-20T23:33:19.922633912Z
status: valid
file: .compozy/tasks/ux-foundation/task_02.md
line: 41
severity: high
author: unknown
---

# Issue 002: Task02 has no required visual-contract evidence

## Review Comment

Task02 requires durable PASS bundles for VC-01 through VC-06 at `.compozy/tasks/ux-foundation/evidence/visual/task_02/<contract-id>/`, including reference, implementation, side-by-side, diff, comparison, and review artifacts. Repository inspection finds only `evidence/visual/task_04`; the entire task_02 evidence directory is absent. Therefore the normative transcript and responsive-layout states have no visual-parity verification despite task_02 listing it as a required deliverable and success criterion.

## Triage

- Decision: `VALID`
- Root cause: task_02's deterministic Ratatui snapshot coverage existed, but no
  visual-contract evidence bundles were captured and retained under the
  required task-specific evidence path. The two disclosure/debug contract
  states also lacked snapshots at their exact contract viewports.
- Fix: add exact-size snapshot coverage for 100x30 collapsed/expanded tool
  disclosure and 120x40 raw-debug presentation, then capture the six
  task_02 contract bundles from the canonical deterministic snapshots.
- Verification owner: `cargo test -p batuta-tui --test render`, the workspace
  CI checks, and the checked visual artifacts in this issue's scoped bundle.
