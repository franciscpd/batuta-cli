# Compozy OpenAPI pin

The filtered pin was generated from Compozy commit `a35eda6d`
(`v0.3.0-beta.16-9-ga35eda6d`) with:

```sh
scripts/pin-contract.sh /home/franciscpd/Projects/compozy a35eda6d
```

The full original `openapi/compozy.json` SHA-256 was
`3de64bdd8c06c806c10c867b16697063dc7ec4207c6e7effa53c12739e22899b`.
`info.version` in the OpenAPI document is the constant `1.0.0`; it is not the
daemon compatibility version. `contract/routes.txt` is the authoritative list
of client routes. The script uses a detached worktree and sorted JSON, so
re-running against the same source must produce no diff.

Task retry uses the pinned task-run enqueue operation:
`POST /api/tasks/{id}/runs`. Task 2 re-runs the pin after the delivery-2 route
list is complete.
