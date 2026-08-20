---
round: 4
round_created_at: 2026-08-20T21:17:07.67088399Z
status: valid
file: .compozy/tasks/ux-foundation/task_04.md
line: 39
severity: high
author: unknown
---

# Issue 002: Required onboarding visual-contract evidence is absent

## Review Comment

Task04 requires a complete evidence bundle for VC-01 through VC-04 at `.compozy/tasks/ux-foundation/evidence/visual/task_04/<contract-id>/` (reference, implementation, side-by-side, diff, comparison, and review files). Repository inspection found no evidence directory or files. Consequently the normative onboarding states have no visual-parity verification against `_uiux.md`. Produce and validate all four required bundles before finalizing the task.

## Triage

- Decision: `VALID`
- Root cause: no VC-01–VC-04 evidence bundles had been produced, and the
  deterministic onboarding renderer still differed from the normative contract:
  it omitted `Name`/`Path` labels, used a generic title and undocumented chooser
  key, retained confirmation chrome after recovery, and clipped long fallback
  text instead of wrapping it.
- Remediation: aligned the overlay state text and title with `_uiux.md`/`_dx.md`,
  bound the chooser to `w`, clear confirmation state on all registration recovery
  outcomes, and added deterministic 100x30 snapshot coverage for candidate,
  confirmation, progress, unsupported, and failure states. The four required
  evidence bundles are now under `evidence/visual/task_04/`.
