---
status: completed
title: Fresh-runner boundary bootstrap
type: infra
complexity: low
---

# Task 7: Fresh-runner boundary bootstrap

## Overview

Fixes the repository boundary check so it succeeds on a fresh GitHub Actions
runner without relying on a pre-populated Cargo registry cache. The check must
still enforce every existing crate dependency boundary and fail when a
forbidden dependency is introduced.

<critical>
- ALWAYS READ `_spec.md` and its catalogs (`_user_stories.md`, `_dx.md`, `_tests.md`) before starting
- PRESERVE every existing boundary rule; this task changes bootstrap reliability, not architecture
- KEEP the correction at the narrowest responsible layer
- TESTS REQUIRED — implement every test case assigned in ## Tests
</critical>

<requirements>
- MUST make `scripts/check-boundaries.sh` deterministic on a clean runner with an empty Cargo cache.
- MUST NOT weaken, remove, or bypass any existing dependency-boundary assertion.
- MUST use the committed lockfile for dependency resolution.
- MUST keep the script directly runnable by contributors and by `.github/workflows/ci.yml`.
- MUST prove both the success path and rejection of a representative forbidden dependency.
- MUST NOT introduce a repository-local dependency cache or commit generated Cargo artifacts.
</requirements>

## Subtasks

- [x] 7.1 Reproduce the fresh-runner failure with an empty temporary Cargo home.
- [x] 7.2 Correct the boundary-check bootstrap without changing its architectural rules.
- [x] 7.3 Add regression coverage for a clean cache and for a forbidden dependency.
- [x] 7.4 Run the exact boundary command used by CI from a clean environment.

## Implementation Details

The PR check failed before evaluating any boundary because `cargo metadata
--offline` could not resolve `async-stream` on a fresh runner. Resolve that
cache assumption while preserving the script as the single source of boundary
enforcement.

### Relevant Files

- `scripts/check-boundaries.sh` — boundary assertions and the failing Cargo metadata bootstrap.
- `.github/workflows/ci.yml` — invokes the boundary script on GitHub's fresh runner.
- `Cargo.lock` — committed dependency resolution that must remain authoritative.
- `Cargo.toml` and crate manifests — inputs inspected by the boundary check.

### Dependent Files

- None.

## Deliverables

- Boundary checks run successfully on a fresh runner with an empty Cargo cache.
- Existing forbidden-dependency detection remains effective.
- Regression coverage assigned below is implemented and passing.

## Tests

Cases assigned from `_tests.md` — read the full definition before implementation.

- [x] IT-022 — boundary check bootstraps from an empty Cargo cache and still rejects a forbidden dependency

## Success Criteria

- `scripts/check-boundaries.sh` passes in a clean environment using the committed lockfile.
- The regression test demonstrates that a forbidden dependency still fails the check.
- The GitHub Actions `boundaries` job no longer depends on a warm Cargo cache.
