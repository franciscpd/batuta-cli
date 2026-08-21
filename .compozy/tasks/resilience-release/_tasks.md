---
schema_version: "compozy.tasks/v2"
workflow: resilience-release
graph:
  nodes:
    - id: task_01
      file: task_01.md
    - id: task_02
      file: task_02.md
    - id: task_03
      file: task_03.md
    - id: task_04
      file: task_04.md
    - id: task_05
      file: task_05.md
    - id: task_06
      file: task_06.md
    - id: task_07
      file: task_07.md
    - id: task_08
      file: task_08.md
  edges:
    - from: task_01
      to: task_03
    - from: task_05
      to: task_06
---

# Resilience & Release (Delivery 3) Task List

Eight tasks decompose GitHub issue #3 / `_spec.md` plus the CI hardening
found during the first real PR run. `task_03` depends on
`task_01` (doctor's catalog probe must reflect the fixed retry behavior,
not the removed Fatal-on-503 fallback). `task_06` depends on `task_05`
(the release pipeline's changelog extraction needs `cliff.toml` and the
retroactive `CHANGELOG.md` to already exist). `task_07` and `task_08` are
independent hardening tasks discovered from the real PR checks and may run
in parallel.

| Task | Title | Type | Complexity |
| --- | --- | --- | --- |
| task_01 | Draining state & stream resilience | backend | high |
| task_02 | Startup retry screen | backend | medium |
| task_03 | `batuta doctor` stream health | backend | medium |
| task_04 | CI contract test pinning | infra | low |
| task_05 | Publish-prep | chore | low |
| task_06 | Release pipeline | infra | high |
| task_07 | Fresh-runner boundary bootstrap | infra | low |
| task_08 | Retry-screen PTY harness reliability | bugfix | medium |
