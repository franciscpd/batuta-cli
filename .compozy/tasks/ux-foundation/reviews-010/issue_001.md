---
round: 10
round_created_at: 2026-08-20T22:49:33.079242672Z
status: valid
file: crates/batuta-tui/src/app/update/stream.rs
line: 130
severity: high
author: unknown
---

# Issue 001: Off-tail transcript deltas never update the unseen count

## Review Comment

When `follow` is false, the successful delta path only marks the cache dirty and returns; it neither preserves/increments `FooterState::NewBelow` nor records the number of raw updates. Reproduce by scrolling away from the tail, then delivering any `TranscriptEvent::Delta`: no visible `N new updates` indicator appears, including when a collapsed group grows. This violates the follow-tail contract in `_spec.md:107-110` and UT-717/IT-701 in `_tests.md:127-131,190-192`. Increment the unseen count from incoming raw delta entries without moving the selected source anchor.

## Triage

- Decision: `VALID`
- Root cause: the successful `TranscriptEvent::Delta` branch invalidated the
  presentation cache but did not update `FooterState` when follow-tail was
  suspended, unlike the snapshot branch. Consequently, in-place updates to raw
  entries (including collapsed operational groups) were invisible to the
  unseen-update indicator.
- Fix approach: count each received raw delta entry before applying the delta;
  when follow-tail is off, accumulate that count in `FooterState::NewBelow`
  without changing the selected presentation row. Add regression coverage for
  an in-place collapsed tool-group update.
- Verification: `cargo fmt --check`; focused `detail_session` regression;
  `cargo clippy --workspace --all-targets -- -D warnings`; permitted workspace
  tests (`cargo test --workspace --exclude compozy-client`, 336 passed); and
  `scripts/check-boundaries.sh` all passed. The client contract test was not
  run because its guard rejects checkouts containing `.compozy`, and the fix
  does not touch that client surface.
