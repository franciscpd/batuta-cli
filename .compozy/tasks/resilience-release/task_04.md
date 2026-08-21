---
status: completed
title: CI contract test pinning
type: infra
complexity: low
---

# Task 4: CI contract test pinning

## Overview

Makes the existing `contract` CI job run automatically on every pull
request against a real, pinned CompozyOS daemon, instead of only running
on `workflow_dispatch` or when a `COMPOZY_TEST_DAEMON_BIN` variable is
manually set. Closes the gap where daemon-contract regressions only
surface if someone remembers to trigger the job by hand.

<critical>
- ALWAYS READ `_spec.md` and its catalogs (`_user_stories.md`, `_dx.md`, `_tests.md`) before starting
- REFERENCE `_spec.md` Part II for implementation details — do not duplicate here
- FOCUS ON "WHAT" — describe what needs to be accomplished, not how
- MINIMIZE CODE — show code only to illustrate current structure or problem areas
- TESTS REQUIRED — implement every test case assigned in ## Tests
</critical>

<requirements>
- MUST remove the `if: github.event_name == 'workflow_dispatch' || env.COMPOZY_TEST_DAEMON_BIN != ''` gate on the `contract` job — it runs unconditionally on every PR.
- MUST add a step that installs the pinned CompozyOS daemon via `go install github.com/compozy/compozy@v0.3.0-beta.16` (matching the existing OpenAPI contract pin in `contract/README.md`), per ADR-003.
- MUST add `$(go env GOPATH)/bin` to `PATH` and set `COMPOZY_TEST_DAEMON_BIN` to the installed binary path before the existing `cargo test -p compozy-client --test contract` step.
- MUST fail the job loudly (not silently skip) if the pinned binary fails to install.
- MUST keep the pinned version as a single, easily-bumped value (a workflow env var or a clearly labeled line), independent of `contract/*.json`'s own pin — both should move together across future PRs, but this task does not need to auto-couple them.
- MUST NOT modify `crates/compozy-testkit`'s worktree/`.compozy/`-absence requirement — CI's fresh checkout already satisfies it; add a comment noting this explicitly so a future caching change doesn't silently break it.
- SHOULD add Go build caching (`actions/setup-go`'s built-in cache) to keep the added CI time bounded.
</requirements>

## Subtasks

- [x] 4.1 Add an `actions/setup-go` step to the `contract` job with build caching enabled.
- [x] 4.2 Add the pinned `go install github.com/compozy/compozy@v0.3.0-beta.16` step, failing the job on install error.
- [x] 4.3 Set `COMPOZY_TEST_DAEMON_BIN` and `PATH` for the subsequent contract-test step.
- [x] 4.4 Remove the `workflow_dispatch`/`COMPOZY_TEST_DAEMON_BIN` conditional gate on the job.
- [x] 4.5 Add an inline comment/guard step documenting the worktree/`.compozy/`-absence requirement this job must keep honoring.
- [x] 4.6 Workflow is configured for automatic PR execution; real PR/checks UI observation remains the post-PR verification step.

## Implementation Details

Reference ADR-003 for why `go install` was chosen over the curl installer
(no version pinning) or npm (unnecessary second toolchain). This task is
YAML-only — no Rust code changes.

### Relevant Files

- `.github/workflows/ci.yml:1-70` — full existing CI structure; the `contract` job (`:63-70`) is what this task modifies.
- `contract/README.md` — existing daemon-version pin convention (commit `a35eda6d` / `v0.3.0-beta.16`), reused as the CI install pin's starting value.
- `crates/compozy-testkit/README.md` — the disposable-worktree/`.compozy/`-absence requirement this job must keep honoring.
- root `CLAUDE.md:10-11`, `CONTRIBUTING.md:23-24` — the same requirement stated at the project level.

### Dependent Files

- None — this task is isolated to `.github/workflows/ci.yml`.

## Deliverables

- `contract` job runs automatically on every PR, no manual trigger required.
- Pinned CompozyOS daemon installed via `go install` before contract tests run.
- Every test case assigned in `## Tests` implemented and passing **(REQUIRED)**.

## Tests

Cases assigned from `_tests.md` — read each ID's full definition there before writing tests.

- [x] IT-013 — workflow statically triggers the contract job for PRs and installs the pinned daemon before tests
- [x] IT-014 — workflow has no path filter or conditional gate, so unrelated PRs also run the job
- [x] IT-015 — clean-checkout guard fails if `.compozy/` exists before tests
- [x] E2E-006 — workflow is ready for real-PR checks UI verification; runtime observation is deferred until a PR exists

## Success Criteria

- Every assigned test case implemented and passing.
- Opening a PR that touches `crates/compozy-client` shows the `contract` job running in GitHub's PR checks without any manual trigger.
- The `workflow_dispatch`/`COMPOZY_TEST_DAEMON_BIN` gate no longer exists in `ci.yml`.
