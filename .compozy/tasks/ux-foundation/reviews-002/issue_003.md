---
round: 2
round_created_at: 2026-08-20T20:19:29.644016395Z
status: valid
file: crates/batuta-tui/src/render_cache.rs
line: 90
severity: high
author: unknown
---

# Issue 003: Transcript presentation still hides raw evidence permanently

## Review Comment

The renderer replaces every part larger than1 MiB with `[part too large: …]`, and separately truncates payloads after200 lines. `TranscriptView` has no global debug/raw-presentation state or cache-key input to bypass those limits. Consequently a large tool/error payload cannot be inspected in full even after expansion, violating the lossless debug requirement and the contract that oversized raw payloads be pageable/wrapped rather than discarded. Implement derived grouping/debug presentation that preserves raw entries and makes all payload content reachable; the task’s UT-709–715 and IT/E2E debug tests are also still absent.

## Triage

- Decision: `valid`
- Root cause: the render cache discarded any serialized part over 1 MiB and
  `append_truncated` dropped every expanded payload line after 200. Neither
  limit had a route to the source data. The transcript view also lacked the
  specified presentation-only raw-debug state and cache-key dimension.
- Fix: retain and wrap every expanded payload, add global `D` raw-debug
  presentation that serializes each original part in source order, and include
  raw-debug state in cache identity. Focused unit coverage exercises an
  over-1-MiB text part, a 201-line tool payload, and raw serialization.
