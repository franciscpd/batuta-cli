---
status: completed
title: Curate Compozy task history and untrack runtime output
type: chore
complexity: medium
---

# Curate Compozy task history and untrack runtime output

## Overview

Make durable Compozy task contracts shareable while keeping runtime configuration, memory, review rounds, and generated evidence local. The change must reconcile existing Git tracking without deleting local workflow output or rewriting history.

<critical>
- ALWAYS READ `_spec.md` and its catalogs (`_user_stories.md`, `_dx.md`, `_uiux.md` when present, `_tests.md`) before starting
- REFERENCE `_spec.md` Part II for implementation details — do not duplicate here
- FOCUS ON "WHAT" — describe what needs to be accomplished, not how
- MINIMIZE CODE — show code only to illustrate current structure or problem areas
- TESTS REQUIRED — implement every test case assigned in ## Tests
</critical>

<requirements>
- `.compozy` MUST be local by default and MUST allow normal Git tracking only for standard task contracts, task files, and ADR Markdown files.
- `.compozy/config.toml`, `.compozy/workspace.toml`, and `.compozy/memory/**` MUST remain ignored.
- Review rounds and visual evidence MUST remain ignored for every workflow.
- Existing generated review/evidence paths MUST leave the index without deleting their working-tree files.
- Current curated contracts for `ux-foundation`, `resilience-release`, and this workflow MUST be added as eligible shared history.
- The implementation MUST create one commit and MUST NOT rewrite history, push, or open a pull request.
</requirements>

## Subtasks

- [x] Replace the broad `.compozy` ignore rule with the approved allowlist for durable task contracts.
- [x] Verify ignore classification for curated contracts, local configuration, memory, review rounds, and visual evidence.
- [x] Remove currently tracked transient output from the index while preserving local files.
- [x] Add existing curated contracts from each task workflow to the index.
- [x] Verify the staged set contains only policy changes, curated contracts, and removals of transient output.
- [x] Commit the completed hygiene change after all assigned checks pass.

## Implementation Details

Follow `_spec.md` Part II exactly. Use Git index-only operations for tracked transient paths and prove local preservation before committing.

### Relevant Files

- `.gitignore` — repository sharing policy.
- `.compozy/tasks/ux-foundation/` — current curated and transient examples.
- `.compozy/tasks/resilience-release/` — existing curated task-contract corpus.
- `.compozy/config.toml` — representative local configuration.
- `.compozy/workspace.toml` — representative local workspace state.
- `.compozy/memory/` — representative local memory state.

### Dependent Files

- `.compozy/tasks/compozy-artifact-hygiene/_spec.md` — policy contract.
- `.compozy/tasks/compozy-artifact-hygiene/_dx.md` — Git-visible usage contract.
- `.compozy/tasks/compozy-artifact-hygiene/_tests.md` — verification contract.

## Deliverables

- Selective `.compozy` ignore policy.
- Index containing curated task contracts and no generated review/evidence output.
- Local configuration, workspace state, memory, review artifacts, and evidence preserved but untracked.
- One atomic commit for this task.
- Every test case assigned in `## Tests` implemented and passing **(REQUIRED)**.

## Tests

- [ ] UT-800, UT-801, UT-802 — curated standard contracts and ADR classification.
- [ ] UT-803, UT-804, UT-805 — local configuration, generated output, and post-cleanup status.
- [ ] IT-800, IT-801, IT-802, IT-803 — index contents, local preservation, and staged-set audit.
- [ ] E2E-800 — collaborator clone and review journey.

## Success Criteria

- Every assigned test case implemented and passing.
- No local `.compozy` file is deleted.
- No configuration, memory, review, or evidence path remains tracked.
- Every standard task contract is normally trackable.
