# ADR-002: `draining` becomes a batuta-tui-local enum, `compozy-client` keeps returning the raw string

## Status

Accepted

## Date

2026-08-19

## Context

`crates/compozy-client/src/status.rs:11-19` maps a 503 with "daemon is
draining" in its body into `StatusPayload{ daemon.status: "draining" }` —
a raw `String`. Two independent call sites compare it ad hoc:
`crates/batuta-tui/src/app/model.rs:376` / `header.rs:34`, and
`crates/batuta/src/doctor.rs:114`. US-003/US-004 need draining to be a
distinct, reliable UI state; duplicated string comparison is the kind of
drift risk this delivery is explicitly closing (see the catalog-stream
bug found by the same pattern of ad hoc status handling).

## Decision

`compozy-client` keeps `daemon.status` as a plain `String` — it is a
generic client with no batuta-specific types (`CLAUDE.md`), and the
protocol-level status is inherently a string the daemon defines, not an
enum the client should own. `batuta-tui` introduces its own
`DaemonState` enum (`Connected | Draining | Offline`) derived once, in one
place, from `StatusPayload.daemon.status` plus the existing `poll_ok`/
stream-liveness signals — and every consumer (`header.rs`, view dimming,
the new draining banner) reads `Model.daemon_state`, not the raw string.
`crates/batuta/src/doctor.rs` performs its own equivalent single-point
derivation from the `StatusPayload` it receives directly (it does not
share `batuta-tui`'s `Model`, so it cannot import the same enum without
creating a `batuta`→`batuta-tui` dependency, which would invert the
existing crate graph).

## Alternatives Considered

### Alternative 1: Move the enum into `compozy-client`

- **Description**: Define `DaemonState` in `compozy-client` so both
  `batuta` and `batuta-tui` share one source of truth.
- **Pros**: True single source of truth across both consumers.
- **Cons**: `compozy-client` "never contains batuta-specific names"
  (`CLAUDE.md`) — `DaemonState` as modeled here (what a *batuta* UI does
  with draining) is a batuta-side interpretation of a generic status
  string, not a client-transport concern.
- **Why rejected**: Violates the crate-boundary rule stated directly in
  `CLAUDE.md`; the string-to-enum mapping is cheap enough that duplicating
  the *derivation logic* (not the underlying data) in two small call
  sites is acceptable, matching the existing precedent that `doctor.rs`
  and `batuta-tui` already independently interpret the same
  `StatusPayload`.

## Consequences

### Positive

- Single derivation point inside `batuta-tui` removes the duplicated
  ad hoc string comparison that caused drift risk.
- `compozy-client` stays exactly as generic as the project boundary
  requires.

### Negative

- `doctor.rs` and `batuta-tui` each derive their own enum from the same
  string — two implementations of one small `match`, not deduplicated.

### Risks

- If a third consumer of `StatusPayload` appears later needing the same
  derivation, revisit whether a shared (non-`compozy-client`) helper crate
  is warranted. Not needed for two consumers.

## Implementation Notes

- `DaemonState` derivation: `"draining" → Draining`; unreachable/`!poll_ok`
  with stale streams → `Offline` (existing `offline()` predicate,
  `header.rs:4-13`); otherwise `Connected`.

## References

- `_spec.md` US-003, US-004; `_user_stories.md` Draining UX.
