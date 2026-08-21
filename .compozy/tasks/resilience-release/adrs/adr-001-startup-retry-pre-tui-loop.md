# ADR-001: Startup retry screen as a pre-TUI loop, not a `Model`/`Client` restructure

## Status

Accepted

## Date

2026-08-19

## Context

`crates/batuta/src/app.rs` currently calls `probe()` once (3s timeout) and,
on failure, prints to stderr and exits before ever constructing a `Model`
or entering `batuta_tui::runtime::run(model, client, &mut terminal)`
(`runtime/mod.rs:458`). There is no existing concept of a `Model` without a
live `Client` — `AppMode` only has `Full`/`TailOnly` (`model.rs:17-22`),
both of which assume an established connection. US-001/US-002 require a
retry screen when the daemon is unreachable at startup.

## Decision

Add a small pre-TUI retry loop in `crates/batuta/src/app.rs` that owns the
terminal (via the same `ratatui` setup already used for the main run),
redraws a static "connecting… (attempt N) / last error: …" screen, and
retries `probe()` on a flat 3s cadence. On success, it tears down nothing
terminal-mode-wise and falls through to the existing boot path (construct
`Client`, `Model`, call `runtime::run` exactly as today). `q`/quit exits
the process directly from this loop with exit code 0.

## Alternatives Considered

### Alternative 1: `Option<Client>` in `Model`

- **Description**: Restructure `Model`/`RuntimeClient`/the update loop to
  accept a disconnected state as a real `AppMode`, so the retry screen
  gets full TUI chrome (dimmed header/panels) from the first frame.
- **Pros**: Architecturally uniform — draining/offline/startup-absent all
  become the same kind of `Model` state; more reusable if a "reconnect
  mid-session with full UI" need ever emerges beyond what already exists.
- **Cons**: Touches `Model` construction, the update loop, and every call
  site that currently assumes a live `Client` — large surface for a
  screen that only exists for a few seconds at startup, before any
  session/workspace data exists to render anyway (there's nothing to dim;
  panels would be empty regardless).
- **Why rejected**: The dimming pattern (US-003/US-004) already works and
  is unaffected — it applies to a live `Model` losing connectivity
  mid-session, not to boot-time absence. Restructuring `Client` to be
  optional everywhere is speculative scope for a screen with no session
  data to show; violates "smallest sufficient" given a pre-TUI loop fully
  satisfies US-001/US-002's acceptance criteria.

## Consequences

### Positive

- Zero changes to `Model`, `Cmd`, `RuntimeClient`, or the update loop —
  the entire existing live-session architecture is untouched.
- Retry screen reuses the same terminal setup/teardown as the main run,
  so there's no flicker or mode-switch when the transition happens.

### Negative

- Two independent rendering code paths exist for "connecting" state (this
  loop's static screen vs. the live `Model`'s dimmed panels) instead of
  one unified one. Acceptable: they represent genuinely different
  moments (no data yet vs. data going stale).

### Risks

- If a future delivery wants the retry screen to show partial session
  data (e.g., last-known-good cache), this ADR's boundary would need
  revisiting. Not a requirement here (Non-Goals).

## Implementation Notes

- The loop lives entirely in `crates/batuta/src/app.rs`; no new crate or
  module needed under `batuta-tui`.
- Reuses `probe()` from `crates/batuta/src/main.rs:90-98` — no duplicate
  probing logic.

## References

- `_spec.md` US-001, US-002; `_dx.md` Golden Path.
