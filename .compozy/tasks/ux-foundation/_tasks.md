---
schema_version: "compozy.tasks/v2"
workflow: ux-foundation
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
  edges:
    - from: task_03
      to: task_04
---

# UX Foundation Task List

| Task | Type | Complexity | Scope | Assigned tests |
| --- | --- | --- | --- | --- |
| task_01 | frontend | medium | Semantic terminal theme and configuration | 7 |
| task_02 | frontend | high | Lossless transcript presentation and adaptive layout | 25 |
| task_03 | backend | medium | Generic registration client and deterministic workspace resolution | 7 |
| task_04 | frontend | high | Workspace onboarding state machine | 12 |

`task_04` begins only after the generic client and resolver contract in `task_03` is available. The other tasks have no graph dependency.
