# Developer Experience: Compozy Artifact Hygiene

Public-surface contract for sharing curated Compozy task history while keeping
runtime state local. Companion to `_spec.md` and `_tests.md`.

## Golden Path

After the policy ships, a contributor verifies what a PR can share:

```console
$ git check-ignore --no-index .compozy/tasks/ux-foundation/_spec.md
$ echo $?
1

$ git check-ignore --no-index .compozy/config.toml
.compozy/config.toml
$ echo $?
0

$ git status --short .compozy
```

The first path is eligible for Git tracking. The configuration path is ignored. The status output contains no generated review or visual-evidence path unless a contributor intentionally changes a curated task contract.

## Repository Policy

The following files are shareable from every `.compozy/tasks/<workflow>/` directory:

```text
_spec.md
_user_stories.md
_dx.md
_uiux.md
_tests.md
_tasks.md
task_*.md
adrs/*.md
```

All other `.compozy` paths are local, including:

```text
.compozy/config.toml
.compozy/workspace.toml
.compozy/memory/**
.compozy/tasks/**/reviews-*/**
.compozy/tasks/**/evidence/**
```

## CLI

To inspect tracked task contracts:

```console
$ git ls-files .compozy/tasks/ux-foundation
.compozy/tasks/ux-foundation/_spec.md
.compozy/tasks/ux-foundation/_tasks.md
.compozy/tasks/ux-foundation/task_01.md
...
```

To inspect local-only state:

```console
$ git check-ignore --no-index .compozy/tasks/ux-foundation/reviews-001/issue_001.md
.compozy/tasks/ux-foundation/reviews-001/issue_001.md
```

## Errors

| Condition | Observable result | Action |
| --- | --- | --- |
| A curated contract is unexpectedly ignored | `git check-ignore --no-index <path>` exits `0` | Correct the allowlist before committing |
| A local path is unexpectedly trackable | `git check-ignore --no-index <path>` exits `1` | Add or correct a local-only ignore rule before committing |
| A generated path remains tracked | `git ls-files <path>` prints the path | Remove it from the index without deleting the local file |
