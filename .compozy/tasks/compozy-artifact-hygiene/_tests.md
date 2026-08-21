# Test Specification: Compozy Artifact Hygiene

Canonical test contract for Compozy artifact sharing policy. Companion to
`_spec.md`, `_user_stories.md`, and `_dx.md`.

## Strategy

- Harness: Git commands against this checkout; no application code harness.
- Execution: run `git check-ignore --no-index`, `git ls-files`, and `git status --short` after the index reconciliation.
- Conventions: use representative existing `ux-foundation` paths and verify both classification and local-file preservation.

## Coverage Matrix

| Source | Behavior | Unit | Integration | E2E |
| --- | --- | --- | --- | --- |
| US-001 | Durable contracts are shareable | UT-800, UT-801 | IT-800 | — |
| US-001.EC-1 | Optional artifacts do not affect classification | UT-802 | — | — |
| US-002 | Configuration and generated state stay local | UT-803, UT-804 | IT-801 | — |
| US-002.EC-1 | Untracking preserves local files | — | IT-802 | — |
| US-003 | PR-oriented worktree is free of generated churn | UT-805 | IT-803 | E2E-800 |

## Unit Tests

### Ignore classification (Spec: Core Interfaces)

- **UT-800** (happy): `git check-ignore --no-index` on `_spec.md`, `_user_stories.md`, `_dx.md`, `_uiux.md`, `_tests.md`, `_tasks.md`, and `task_01.md` exits `1` for representative workflows.
- **UT-801** (happy): `git check-ignore --no-index` on `adrs/adr-001.md` exits `1` when the file exists.
- **UT-802** (boundary): an absent optional `_uiux.md` or ADR does not require a matching runtime file for the allowlist to classify existing standard contracts.
- **UT-803** (error): `git check-ignore --no-index` on `.compozy/config.toml`, `.compozy/workspace.toml`, and `.compozy/memory/MEMORY.md` exits `0`.
- **UT-804** (error): `git check-ignore --no-index` on `reviews-001/issue_001.md` and `evidence/visual/task_01/VC-01/review.md` exits `0`.
- **UT-805** (state): `git status --short .compozy` contains no generated review/evidence path after reconciliation.

## Integration Tests

### Index reconciliation (Spec: Development Sequencing)

- **IT-800**: `git ls-files .compozy/tasks/ux-foundation` lists the curated companion documents and task files.
- **IT-801**: `git ls-files` lists no path under `.compozy/config.toml`, `.compozy/workspace.toml`, `.compozy/memory/`, `reviews-*`, or `evidence/`.
- **IT-802**: after index-only removal, a representative locally modified review issue and evidence file still exists in the working tree.
- **IT-803**: `git diff --cached --name-only` contains only `.gitignore` plus allowlisted curated task contracts and removals of formerly tracked transient artifacts.

## End-to-End Tests

### Collaborator review journey (US-001, US-003)

- **E2E-800**: from a clean clone of the resulting commit, list tracked `.compozy/tasks/ux-foundation` files and inspect `_spec.md`; verify no configuration, memory, review, or evidence path is present in the Git tree.
