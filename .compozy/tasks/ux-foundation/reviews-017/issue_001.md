---
round: 17
round_created_at: 2026-08-21T00:33:03.53689718Z
status: valid
file: crates/batuta-tui/src/app/update/api.rs
line: 239
severity: medium
author: unknown
---

# Issue 001: Refresh clears the unsupported-daemon fallback

## Review Comment

After `AddWorkspaceOutcome::Unsupported`, press `r` and return an empty workspace catalog. The response handler replaces the required copyable `compozy workspace add<path>` fallback with “this directory is not registered; add it, refresh, or choose a workspace” (lines239–255), while `registration_complete` still blocks `a`/Enter (keys.rs:524–527). The operator is left unable to add through the TUI and without the required command to run externally. Preserve the unsupported fallback after an empty refresh; only switch/boot when the canonical root is found. This violates `_dx.md:69–87` and UT-731 (`_tests.md:170–173`).

## Triage

- Decision: `VALID`
- Root cause: the empty-catalog response treated every onboarding refresh as a generic
  unregistered-directory state. `Unsupported` had no durable state distinct from a successful
  registration, so the response overwrote its mandatory external `compozy workspace add` fallback
  while registration remained complete and the add keys stayed blocked.
- Fix: record unsupported registration explicitly in onboarding state and preserve its fallback on
  empty refreshes; catalog entries matching the canonical candidate root still switch and boot.
- Scope: `model.rs` stores the distinct state, and `keys.rs` avoids replacing the fallback while a
  read-only refresh is in flight; both are required to prevent either side of the transition from
  discarding the command.
- Regression coverage: `ut_731_unsupported_onboarding_preserves_the_fallback_after_an_empty_refresh`
  exercises Unsupported -> refresh -> empty catalog and asserts the command remains visible while
  further registration writes remain blocked.
