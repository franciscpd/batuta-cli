---
status: completed
title: Publish-prep
type: chore
complexity: low
---

# Task 5: Publish-prep

## Overview

Closes the publish-readiness gaps surfaced during the delivery-2 GitHub
publish: adds the missing dual-license files, fixes the README's broken
"Contributing" link, and generates a retroactive `CHANGELOG.md` covering
the project's full history via `git-cliff`. This task's `cliff.toml` and
initial `CHANGELOG.md` are a hard prerequisite for `task_06`'s release
pipeline.

<critical>
- ALWAYS READ `_spec.md` and its catalogs (`_user_stories.md`, `_dx.md`, `_tests.md`) before starting
- REFERENCE `_spec.md` Part II for implementation details — do not duplicate here
- FOCUS ON "WHAT" — describe what needs to be accomplished, not how
- MINIMIZE CODE — show code only to illustrate current structure or problem areas
- TESTS REQUIRED — implement every test case assigned in ## Tests
</critical>

<requirements>
- MUST add `LICENSE-MIT` and `LICENSE-APACHE` at the repo root with standard, unmodified license text, matching `Cargo.toml`'s `license = "MIT OR Apache-2.0"` declaration.
- MUST verify every workspace member crate's `Cargo.toml` `license` field is consistent with the root declaration — no crate may declare a license the repo doesn't carry the text for.
- MUST replace the README "Contributing" section's link to the git-ignored, nonexistent `.compozy/tasks/mvp-tui/_dx.md` with a link to `CONTRIBUTING.md`, preserving the intent (toolchain, pre-PR checks, crate-boundary rules) — not just deleting the link.
- MUST add `cliff.toml` with `conventional_commits = true` and `filter_unconventional = true`.
- MUST generate `CHANGELOG.md` retroactively, covering the full commit history including an entry for `v0.1.0-beta.1`, not just entries from this delivery forward.
- MUST NOT hand-write changelog entries — the file is `git-cliff`-generated, even for the retroactive pass.
- SHOULD mark `batuta-tui` and `compozy-client`'s `Cargo.toml` with `publish = false` if not already set, since only the `batuta` binary is meant to ship (informs `task_06`'s `dist-workspace.toml`, does not itself configure `cargo-dist`).
</requirements>

## Subtasks

- [x] 5.1 Add `LICENSE-MIT` and `LICENSE-APACHE` (standard unmodified text) at the repo root.
- [x] 5.2 Audit every workspace member `Cargo.toml`'s `license` field for consistency with the root declaration; fix any drift.
- [x] 5.3 Replace the broken README link with a `CONTRIBUTING.md` link, confirming `CONTRIBUTING.md` already covers the original link's intent (or extend it minimally if a real gap exists).
- [x] 5.4 Add `cliff.toml` with the required settings.
- [x] 5.5 Run `git-cliff` retroactively against full project history to generate the initial `CHANGELOG.md`, including a `v0.1.0-beta.1` entry.
- [x] 5.6 Set `publish = false` on `batuta-tui` and `compozy-client` if not already present.
- [x] 5.7 Implement the assigned unit test (license-field consistency check).
- [x] 5.8 Implement the assigned E2E verification (repo-root publish-prep review).

## Implementation Details

Reference `_spec.md` Part II → Assumptions and Defaults for the license-
text-source and platform-target defaults, and US-010/US-011/US-012 in
`_user_stories.md` for the exact acceptance criteria. This task has no
code-behavior changes — it is entirely file additions and one link edit.

### Relevant Files

- root `Cargo.toml:9-13` — workspace `license`, `version`, `edition` declaration; the source of truth `LICENSE-MIT`/`LICENSE-APACHE` must match.
- `crates/batuta/Cargo.toml`, `crates/batuta-tui/Cargo.toml`, `crates/compozy-client/Cargo.toml`, `crates/compozy-testkit/Cargo.toml` — each workspace member's own `license`/`publish` fields to audit; `compozy-testkit` already has `publish = false`.
- `README.md:100-105` — the exact broken-link section to fix.
- `CONTRIBUTING.md` — existing dev-workflow content (Conventional Commits, toolchain, pre-PR checks, crate-boundary rules, contract-test worktree warning); the new link target, already covers the original `_dx.md` link's intent.

### Dependent Files

- `task_06` (Release pipeline) — consumes `cliff.toml` and `CHANGELOG.md` directly; this task's output is a hard input to that task, not just a related file.

## Deliverables

- `LICENSE-MIT`, `LICENSE-APACHE` at repo root, consistent with every crate's `license` field.
- Working README "Contributing" link.
- `cliff.toml` and a retroactively generated `CHANGELOG.md` covering full project history.
- Every test case assigned in `## Tests` implemented and passing **(REQUIRED)**.

## Tests

Cases assigned from `_tests.md` — read each ID's full definition there before writing tests.

- [x] UT-021 — every workspace member's `license` field matches the root declaration
- [x] E2E-009 — repo root shows LICENSE files, working README link, and a `CHANGELOG.md` `v0.1.0-beta.1` entry

## Success Criteria

- Every assigned test case implemented and passing.
- `LICENSE-MIT` and `LICENSE-APACHE` render correctly on GitHub and match `Cargo.toml`'s declaration.
- The README's "Contributing" link resolves to a tracked, rendering file — no 404 on GitHub.
- `CHANGELOG.md` exists with a `v0.1.0-beta.1` entry and is entirely `git-cliff`-generated (no hand-written entries).
