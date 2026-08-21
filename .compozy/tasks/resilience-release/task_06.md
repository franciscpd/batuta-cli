---
status: pending
title: Release pipeline
type: infra
complexity: high
---

# Task 6: Release pipeline

## Overview

Builds the PR-gated release pipeline the operator confirmed: a
`release-plan` CI job that opens/updates a standing release PR with a
version bump and `git-cliff`-generated changelog diff, and a
`cargo-dist`-generated `release.yml` that builds and publishes
checksummed Linux/macOS binaries once that PR is merged and tagged. No
tag, build, or publish happens without a human merging the release PR.
Depends on `task_05` for `cliff.toml` and the existing `CHANGELOG.md`.

<critical>
- ALWAYS READ `_spec.md` and its catalogs (`_user_stories.md`, `_dx.md`, `_tests.md`) before starting
- REFERENCE `_spec.md` Part II for implementation details — do not duplicate here
- FOCUS ON "WHAT" — describe what needs to be accomplished, not how
- MINIMIZE CODE — show code only to illustrate current structure or problem areas
- TESTS REQUIRED — implement every test case assigned in ## Tests
</critical>

<requirements>
- MUST NOT adopt `release-plz` or any third release-automation tool not named in the issue — assemble the PR-gate from `git-cliff`, `cargo-dist`, and `peter-evans/create-pull-request` only, per ADR-004.
- MUST add a `release-plan` job (triggered on push to `main`) that runs `git-cliff --bump` and opens/updates a single standing PR (`release: vX.Y.Z`) with the `Cargo.toml` workspace version bump and the changelog diff.
- MUST update the existing PR (not open a duplicate) when additional commits land on `main` before it is merged.
- MUST exclude commits that don't follow Conventional Commits format from the generated changelog (`filter_unconventional = true`, already set in `task_05`'s `cliff.toml`).
- MUST create the version tag using a repo-scoped PAT stored as a GitHub secret when the release PR is merged — MUST NOT rely on the default `GITHUB_TOKEN`, which does not trigger downstream tag-push workflows.
- MUST NOT create any tag, build, or publish action outside the release-PR-merge path — no code path may tag directly (Safety Invariant #4 in `_spec.md`).
- MUST run `cargo dist init` to generate `dist-workspace.toml` and `.github/workflows/release.yml` (current cargo-dist 0.32.x default format, not legacy `Cargo.toml` metadata).
- MUST list only the `batuta` crate as distributable in `dist-workspace.toml`; `batuta-tui` and `compozy-client` stay internal (their `publish = false` is set in `task_05`).
- MUST target Linux and macOS builds, matching the design doc's existing platform commitment.
- MUST fail the release workflow as a whole (not publish a partial Release) if any targeted platform's build fails.
- MUST document the required PAT secret setup prominently in the task's own PR description or a repo doc — this is a manual operator step this task cannot automate.
</requirements>

## Subtasks

- [x] 6.1 Add the `release-plan` job to a workflow (new or appended to `ci.yml`), running `git-cliff --bump` on every push to `main`.
- [x] 6.2 Wire `peter-evans/create-pull-request` to open/update the standing `release: vX.Y.Z` PR with the version bump and changelog diff.
- [x] 6.3 Configure tag creation on release-PR merge using a repo-scoped PAT secret (document the exact secret name and required scope).
- [x] 6.4 Run `cargo dist init` to generate `dist-workspace.toml` and `.github/workflows/release.yml`, targeting Linux and macOS.
- [x] 6.5 Confirm `dist-workspace.toml` lists only `batuta` as distributable.
- [x] 6.6 Confirm `release.yml`'s generated release-notes extraction correctly reads the matching `CHANGELOG.md` heading.
- [x] 6.7 Verify no code path can tag/build/publish without the release-PR-merge step (review the full workflow graph for this).
- [x] 6.8 Implement all assigned integration tests (release-plan PR creation/update, non-conventional-commit exclusion, tag-trigger via PAT, unmerged-PR-means-no-release, partial-platform-failure handling).
- [x] 6.9 Implement the assigned E2E journeys (full release from merge to published checksummed binaries; binary+checksum download verification).

## Implementation Details

Reference ADR-004 in full for the reasoning behind not adopting
`release-plz` and for the exact three-piece assembly
(`release-plan` job + PAT-gated tag + `cargo-dist`-generated
`release.yml`). Reference `_spec.md` Part II → Development Sequencing for
why this task depends on `task_05` (needs `cliff.toml` and an existing
`CHANGELOG.md` for `cargo-dist`'s release-notes extraction to have
something to read on the first real release) and Known Risks for the PAT
setup risk.

### Relevant Files

- root `Cargo.toml:9-13` — workspace version field; the release-plan job bumps this.
- `.compozy/tasks/resilience-release/task_05.md` output — `cliff.toml` and `CHANGELOG.md`, hard prerequisites for this task.
- `.github/workflows/ci.yml` — existing workflow structure/style to match for the new `release-plan` job (or a new `.github/workflows/release-plan.yml`, implementer's choice, matching repo conventions).
- git tag `v0.1.0-beta.1` — existing tag whose annotation only mentions delivery 1 despite delivery 2 also being in the tagged tree; confirm with the operator during implementation whether this needs correcting or is left as historical record (not blocking, but worth a note in the task's own PR).

### Dependent Files

- `task_05` (Publish-prep) — this task cannot be verified end-to-end without `task_05`'s `cliff.toml`/`CHANGELOG.md` already merged.

### Related ADRs

- [ADR-004: Hand-assembled release pipeline, no third tool](adrs/adr-004-release-pipeline-git-cliff-cargo-dist.md) — the full rationale and glue-assembly design this task implements.

## Deliverables

- `release-plan` job opening/updating a reviewable release PR on every push to `main`.
- PAT-gated tag creation on release-PR merge.
- `dist-workspace.toml` and generated `release.yml` publishing checksummed Linux/macOS binaries.
- Every test case assigned in `## Tests` implemented and passing **(REQUIRED)**.

## Tests

Cases assigned from `_tests.md` — read each ID's full definition there before writing tests.

- [x] IT-016 — commits on `main` open/update a release PR with version bump and changelog diff
- [x] IT-017 — additional commits before merge update the same PR, no duplicate
- [x] IT-018 — non-conventional commit excluded from generated changelog, no corruption
- [x] IT-019 — merging the release PR creates a tag via PAT that triggers `release.yml`
- [x] IT-020 — unmerged release PR results in no tag/build/publish
- [x] IT-021 — a failing platform build fails the release as a whole, no partial publish
- [ ] E2E-007 — release graph and post-publication verifier cover merge → tag → published Release with Linux+macOS binaries and checksums, notes matching `CHANGELOG.md`; real-run observation follows the first human merge
- [ ] E2E-008 — executable post-publication verifier downloads each binary/checksum pair and runs `sha256sum -c`

## Success Criteria

- Every assigned test case implemented and passing.
- No tag, build, or publish occurs anywhere in the pipeline without an explicit human merge of the release PR.
- The first real release run publishes checksummed Linux and macOS binaries with release notes matching the corresponding `CHANGELOG.md` section.
- The required PAT secret setup is documented clearly enough that an operator can complete it without further clarification.
