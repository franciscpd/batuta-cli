# Spike findings

Date: 2026-08-17  
Daemon evidence: `compozy/compozy` `v0.3.0-beta.16-9-ga35eda6d`, commit
`a35eda6d`.

Test coverage: UT-230 validates this document; UT-231 validates the related
design-spec corrections.

## UDS transport

Verdict: yes with caveats

The UDS path works for the MVP, but the result is a hand-written connector
behind `hyper`/`hyper-util`, not the originally proposed `hyperlocal` crate.

Evidence: UT-001, UT-003, UT-004, UT-006, and UT-008 in
`crates/compozy-client/tests/transport.rs`; the fixed-path connector in
`crates/compozy-client/src/transport.rs`; the task 03 WORK note records a live
probe selecting `Uds` against `/home/franciscpd/.compozy/daemon.sock` with TCP
skipped. Contract tests in `crates/compozy-client/tests/contract.rs` passed in
the disposable detached checkout.

Recommendation for the MVP: keep UDS as the first `auto` transport and retain
TCP fallback. Keep the connector fixed to the daemon socket path and preserve
the per-target timeout and probe diagnostics.

## Delta and reset application

Verdict: yes

Fenced deltas apply by `start_sequence`/`sequence`, while a reset snapshot
replaces the transcript state and refreshes the reconnect fences. Unknown
starts request a page refetch rather than guessing.

Evidence: UT-040–UT-055 in `crates/compozy-client/tests/sse.rs` and
`crates/batuta-tui/src/transcript.rs`; the `TranscriptState` tests in
`crates/batuta-tui/tests/state.rs`; snapshots under
`crates/batuta-tui/tests/snapshots/`; task 04 records 18/18 SSE tests and a
live `reset:true` response for an epoch/generation mismatch; task 07 records
the live `resynchronized (fence_missing)` marker.

Recommendation for the MVP: keep the daemon as the source of truth, store
epoch/generation/max sequence in model state, and make reset/refetch visible
as a transient or inline resynchronization status.

## Readability of tool cards and streaming markdown

Verdict: yes with caveats

The terminal rendering is readable at the supported sizes and remains useful
during streaming, but the first MVP should retain truncation, collapsed cards,
and the narrow-terminal guard.

Evidence: UT-120–UT-134 in `crates/batuta-tui/tests/render.rs`; snapshots
`crates/batuta-tui/tests/snapshots/render__screen_80x24.snap`,
`render__screen_120x40.snap`, `render__screen_200x60.snap`,
`render__ut_126_footer_variants_use_exact_copy.snap`, plus marker, permission,
markdown, wrapping, oversize, and unknown-part snapshots in that directory.
Task 07's WORK note records a live 80-column session with real transcript
content, a `resynchronized (fence_missing)` marker, readable footer, and clean
alternate-screen restoration after `q`. The full disposable-checkout test
run passed 153 tests across 19 suites.

Recommendation for the MVP: use the existing pure `views/` renderer with
streaming markdown, collapsed reasoning/tool cards, 200-line payload
truncation, size notices for oversized parts, and the below-20-column warning.

## Dependencies

Used:

- `hyper 1.11` and `hyper-util 0.1.20` — low-level HTTP clients with a small
  custom Unix connector and separate control/SSE pools (ADR-003).
- `eventsource-stream 0.2.3` — SSE framing and parsing.
- `ratatui 0.30.0`, `tui-markdown 0.3.9`, and `tokio 1.53` — terminal views,
  markdown rendering, and the async runtime.

Rejected:

- `hyperlocal 0.9.1` — dormant since 2024-07, targets an older hyper 1.x
  surface, and has an open socket-shutdown issue; the connector is small
  enough to own (ADR-003).
- `reqwest 0.13` — its UDS support is convenient but brings a larger tree,
  requires separate clients for UDS/TCP, and hides the transport behavior the
  spike measures (ADR-003).
- `reqwest-eventsource` — rejected with the reqwest stack; it would couple
  SSE handling to the larger high-level client instead of the existing
  `eventsource-stream` path.
- Original `tui-textarea` dependency — rejected because the first delivery is
  read-only and its composer is outside the spike; no version was retained.

## Design spec corrections

US-019 is closed by the surgical corrections in
`docs/internal/specs/2026-08-17-batuta-cli-design.md`: commit and daemon
version, constant OpenAPI version, non-paginated workspace catalog and
client-side longest-prefix resolution (ADR-002), the structured error
envelope, transcript stream vocabulary/keepalives, `?workspace=`, and
`tool-<toolName>`/`dynamic-tool` parts. The product shape, non-goals, and
deferred list remain unchanged.

## Open items

- Delivery 2 should confirm the daemon's stream vocabulary remains stable as
  more panels consume catalog, loop-run, and log streams.
- The MVP should add UI-level coverage for very long tool payloads and
  pathological markdown beyond the captured snapshots.
- Remote transport, composer behavior, approvals, and clarifications remain
  outside this delivery.
