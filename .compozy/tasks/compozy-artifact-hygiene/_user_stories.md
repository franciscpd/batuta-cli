# User Stories: Compozy Artifact Hygiene

Canonical behavior catalog for repository sharing of Compozy task artifacts.
Companion to `_spec.md`.

## Personas

- **Repository collaborator** — reviews a delivery and needs its durable intent, requirements, and task breakdown without local runtime noise.
- **Local operator** — runs Compozy in a checkout and needs private configuration, memory, generated evidence, and review working state to remain local.

## Story Index

| ID | Feature Area | Persona | Story |
| --- | --- | --- | --- |
| US-001 | Curated task history | Repository collaborator | Read durable task contracts in the repository |
| US-002 | Local runtime state | Local operator | Keep operational state out of shared history |
| US-003 | Reviewable changes | Repository collaborator | Avoid generated-artifact churn in pull requests |

## Curated task history

### US-001: Read durable delivery context

**As a** repository collaborator, **I want** specifications, supporting catalogs, task manifests, task files, and ADRs to be versioned, **so that** I can understand why and how a delivery was planned.

Acceptance criteria:

- AC-1: Given a cloned repository, when I open a task directory, then I can read its curated planning documents without restoring local runtime state.
- AC-2: Given an existing or future task workflow, when its durable planning artifacts change, then Git can include those changes in review.

Edge cases:

- EC-1: A workflow has no ADR or UI contract → the available curated artifacts remain shareable without requiring absent optional files.
- EC-2: A collaborator has no local Compozy configuration → task documentation remains readable and does not require a daemon.
- EC-3: A task is retried or resumed → its durable task contract remains distinct from generated runtime output.

## Local runtime state

### US-002: Keep operational state local

**As a** local operator, **I want** Compozy configuration, workspace state, memory, review rounds, and generated visual evidence to remain local, **so that** sharing the repository does not disclose or synchronize machine-specific operational state.

Acceptance criteria:

- AC-1: Given a local Compozy configuration or workspace state file, when I run Git status, then it is not proposed for commit.
- AC-2: Given review rounds or generated visual evidence, when the workflow updates them, then they do not become tracked changes.
- AC-3: Given currently tracked transient artifacts, when the hygiene change lands, then they remain on disk locally but are removed from the Git index.

Edge cases:

- EC-1: A previously tracked transient file is modified → it stops appearing as a tracked modification after index cleanup while its local content remains.
- EC-2: A fresh checkout has no `.compozy` directory → Git ignore rules do not create or require it.
- EC-3: A local configuration contains credentials or host-specific paths → it remains untracked.

## Reviewable changes

### US-003: Keep pull requests focused

**As a** repository collaborator, **I want** pull requests to exclude generated review and evidence churn, **so that** review focuses on source changes and durable delivery intent.

Acceptance criteria:

- AC-1: Given a pull request for a delivery, when I inspect its file list, then generated review rounds and visual evidence are absent.
- AC-2: Given a durable task contract change, when I inspect its file list, then that document remains visible.

Edge cases:

- EC-1: A visual evidence generator reruns with byte-different output → no generated binary is staged.
- EC-2: Several review rounds run consecutively → no review issue files are staged.
- EC-3: A contributor explicitly force-adds ignored runtime output → normal repository policy still documents it as unsupported and cleanup removes existing tracked output.
