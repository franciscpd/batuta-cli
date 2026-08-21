# ADR-003: CI installs the pinned CompozyOS daemon via `go install github.com/compozy/compozy@<tag>`

## Status

Accepted

## Date

2026-08-19

## Context

The `contract` CI job (`.github/workflows/ci.yml:63-70`) only runs when
`workflow_dispatch` or `COMPOZY_TEST_DAEMON_BIN` is set, and has no step
that installs a daemon binary. US-006 requires it to run automatically on
every PR against a real, pinned daemon. CompozyOS (`github.com/compozy/compozy`)
is a public, open-source Go CLI/daemon with multiple install paths: a curl
verified-installer script (pins only "latest beta", no version parameter),
an npm package (`@compozy/cli@<version>`), and `go install
github.com/compozy/compozy@<release-tag>`. This repo's existing OpenAPI
contract pin (`contract/README.md`) was captured from commit `a35eda6d`
(`v0.3.0-beta.16-9-ga35eda6d`).

## Decision

Add a CI step that runs `go install github.com/compozy/compozy@v0.3.0-beta.16`
(matching the already-pinned contract version, for consistency between
"the API shape we test against" and "the binary we test with"), adds
`$(go env GOPATH)/bin` to `PATH`, and sets `COMPOZY_TEST_DAEMON_BIN` to the
resulting binary path before running `cargo test -p compozy-client --test
contract`. The `workflow_dispatch`/env-var gate on the job is removed — it
now runs unconditionally, per US-006 AC-1. The pinned tag lives as a
single value at the top of the CI step (or a `COMPOZY_PINNED_VERSION`
workflow env var) so bumping it is a one-line change, independent of
`contract/README.md`'s pin (which tracks API *shape*; this tracks the
*binary* used to exercise it — they should move together but are updated
by the same PR, not automatically coupled).

## Alternatives Considered

### Alternative 1: curl verified-installer script

- **Description**: `curl -fsSL https://compozy.com/install.sh | sh`.
- **Pros**: Matches the officially documented "recommended" install path
  for end users.
- **Cons**: Documented as pinning "the latest published beta" with no
  version parameter — not deterministic, contradicts "pinned" requirement
  outright.
- **Why rejected**: Fails the core requirement (a *pinned* daemon), not a
  trade-off question.

### Alternative 2: npm package (`@compozy/cli@<version>`)

- **Description**: `npm install -g @compozy/cli@0.3.0-beta.16`.
- **Pros**: Version-pinnable; npm is already present on GitHub-hosted
  Linux runners.
- **Cons**: Introduces an npm dependency into a Rust-only project's CI for
  a Go binary — an extra toolchain with no other use in this repo.
- **Why rejected**: `go install` is more direct for a Go binary and adds
  no toolchain this CI doesn't already need to reason about beyond Go
  itself (which GitHub-hosted runners also ship); avoids adding npm as a
  CI dependency purely to fetch one binary.

## Consequences

### Positive

- Deterministic, reviewable one-line version pin; no new toolchain beyond
  a `go` setup step.
- Matches the existing contract-pin convention of naming an exact
  CompozyOS commit/tag.

### Negative

- Requires a `go` setup step (`actions/setup-go`) in a CI that is
  otherwise Rust-only — one additional toolchain, unavoidable given
  CompozyOS is a Go project with no prebuilt-binary-per-tag GitHub Release
  asset confirmed in this research pass (only the three install paths
  above were found).

### Risks

- `go install @<tag>` compiles from source on the runner (not a prebuilt
  binary download) — adds CI time proportional to CompozyOS's own build
  time. Mitigate with the existing Rust-toolchain-cache pattern already in
  `ci.yml`, applied to Go's module/build cache too (`actions/setup-go`
  supports this natively).

## Implementation Notes

- Still must be run from a disposable/detached worktree without
  `.compozy/`, per the existing `compozy-testkit` requirement — the CI
  job's checkout is already a fresh clone per run, so this constraint is
  satisfied by GitHub Actions' default checkout behavior; no extra step
  needed, but call this out explicitly in the workflow's comments so it
  isn't accidentally broken by a future caching change that persists
  `.compozy/`.

## References

- `_spec.md` US-006; `contract/README.md` (existing API-shape pin);
  `crates/compozy-testkit/README.md` (worktree requirement).
