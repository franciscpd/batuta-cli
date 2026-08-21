# Spec: Compozy Artifact Hygiene

---

# Part I — Product

## Overview

This delivery separates shareable Compozy task history from machine-local runtime output. It serves repository collaborators who need delivery context and local operators who need configuration, memory, and generated workflow state to stay private and non-disruptive.

## Goals

- Collaborators can read durable specifications, catalogs, task manifests, task files, and ADRs from a clone.
- Local Compozy configuration, workspace state, memory, review rounds, and generated evidence do not appear in normal Git changes.
- Existing transient artifacts leave the index without being deleted from an operator's checkout.

## User Stories

- US-001 — curated task history and delivery context.
- US-002 — local runtime state remains local.
- US-003 — focused pull requests without generated churn.

[Full user stories](_user_stories.md)

## Core Features

### Curated task history

Version durable task contracts under `.compozy/tasks/`, including unified specifications, companion catalogs, task graph manifests, task files, and ADRs when present.

### Local runtime state

Keep Compozy configuration, workspace state, memory, review-round artifacts, and generated visual evidence local to each checkout.

### Index cleanup

Remove already tracked transient artifacts from Git tracking while retaining their local files, so current workflow state is not destroyed.

## Business Rules

- Curated task documentation is shareable repository history; generated workflow output is not.
- Repository ignore rules must make the distinction for all task workflows, not just `ux-foundation`.
- Local configuration and memory are never made shareable by this delivery.
- Removing a transient path from the index must not remove its working-tree file.
- The delivery does not rewrite existing commit history.

## User Experience

A collaborator clones the repository and sees durable task documentation alongside source changes. A local operator runs Compozy normally; configuration, memory, reviews, and evidence remain available locally without appearing in `git status`. Pull requests stay focused on source and intentionally curated delivery contracts.

## High-Level Technical Constraints

- Git's ignore behavior must distinguish untracked durable task documents from ignored local runtime output.
- The policy applies without requiring a running Compozy daemon.
- The cleanup must preserve local files and existing Git history.
- No web surface changes are part of this delivery.

## Non-Goals (Out of Scope)

- Rewriting, deleting, or migrating historical commits.
- Sharing local Compozy configuration, workspace state, memory, credentials, or generated evidence.
- Changing Compozy runtime behavior or review-loop semantics.

## Open Questions

None. The operator confirmed that configurations must remain local and that durable task contracts should be shared.

---

# Part II — Technical

## Executive Summary

Replace the broad `.compozy/` ignore rule with a deny-by-default policy that re-includes only durable task contracts. Remove already tracked generated review and evidence paths from the index without deleting their working-tree files, then add the approved task-contract files to the index.

## MVP Boundary

The MVP is one cleanup task: establish the selective ignore policy, untrack current transient artifacts, track current curated task documents, and prove the resulting index and ignore behavior. No runtime code changes are included.

## Developer Experience

- [Developer experience contract](_dx.md) — Git-visible sharing and local-only behavior.

## System Architecture

The repository root `.gitignore` is the distributed policy surface. `.compozy/tasks/<workflow>/` is partitioned into curated task contracts and ignored runtime output. Git's index is reconciled once for paths already tracked; the local filesystem is not deleted.

## Architectural Boundaries

- `.gitignore` defines repository-wide sharing policy.
- `.compozy/tasks/**` holds task contracts and local workflow artifacts.
- `.compozy/config.toml`, `.compozy/workspace.toml`, and `.compozy/memory/**` remain local and are never added to Git.
- No Rust crate or Compozy runtime definition changes.

## Implementation Design

### Core Interfaces

The public contract is Git path classification:

```text
curated: .compozy/tasks/<workflow>/{_spec,_user_stories,_dx,_uiux,_tests,_tasks}.md
curated: .compozy/tasks/<workflow>/task_*.md
curated: .compozy/tasks/<workflow>/adrs/*.md
local:   every other .compozy path
```

### Data Models

No application data model changes. Git paths are the only classified data; repository rules express the durable/local distinction instead of adding a config key or stored state.

## Impact Analysis

| Component | Impact Type | Description and Risk | Required Action |
| --- | --- | --- | --- |
| `.gitignore` | modified | Selective allowlist replaces broad ignore; incorrect negation could expose local state | Verify representative paths with `git check-ignore --no-index` |
| Git index | modified | Existing generated paths remain on disk but leave tracking | Use index-only removal and verify the working tree retains files |
| `.compozy/tasks/*` | modified | Curated contracts become shareable history | Add only allowlisted documents |

## Extensibility Integration Plan

Compozy extensions, hooks, skills, tools, and runtime protocols are unaffected. The policy applies to their filesystem output only.

## Agent Manageability Plan

Agents and operators inspect the outcome with `git check-ignore --no-index`, `git ls-files`, and `git status --short`. No Compozy native tool surface changes.

## Config Lifecycle

Compozy configuration is explicitly local: `.compozy/config.toml`, `.compozy/workspace.toml`, and `.compozy/memory/**` remain ignored. No configuration value is added, migrated, or shared.

## Testing Approach

Use Git commands as the integration boundary: check representative curated and local paths against ignore rules, assert tracked-path classification with `git ls-files`, and inspect the post-change worktree. No Rust test suite is affected.

## Development Sequencing

### Build Order

1. Define and validate the selective ignore policy against representative paths.
2. Reconcile the index: remove generated artifacts without deleting local files and add curated task contracts.
3. Run the test contract and commit the resulting hygiene change alone.

### Technical Dependencies

Git is available in the repository checkout. No daemon, provider, or network dependency is required.

## Monitoring and Observability

`git status --short`, `git ls-files`, and `git check-ignore --no-index` are the durable verification evidence. No runtime metric or alert applies.

## Technical Considerations

### Key Decisions

- Use a deny-by-default allowlist under `.compozy/tasks/` so new runtime outputs stay local unless explicitly classified as durable.
- Preserve existing local files with index-only removal rather than deleting them.
- Do not rewrite historical commits; the cleanup is one forward commit.

### Known Risks

- An incomplete allowlist could hide a durable artifact; verify every standard task-contract name.
- An overly broad negation could stage local state; verify configuration, memory, reviews, and evidence remain ignored.

## Safety Invariants

1. No local `.compozy` file is deleted by the cleanup.
2. No configuration, workspace state, memory, review output, or generated evidence is tracked after the cleanup.
3. Every allowed task contract is eligible for normal Git tracking.
4. Existing commit history is not rewritten.

## File References

### Repo Files

- `.gitignore` — current broad `.compozy/` policy to replace.
- `.compozy/tasks/ux-foundation/` — current curated contracts and tracked transient examples.
- `.compozy/tasks/resilience-release/` — existing task-contract corpus that must remain shareable.

## Assumptions and Defaults

- Curated files are the six standard companion documents, `task_*.md`, and ADR Markdown files.
- All other `.compozy` paths are local by default.
- Visual evidence is generated workflow output and remains local.

## Architecture Decision Records

No ADR is required: the operator explicitly selected the durable/local policy in this conversation.
