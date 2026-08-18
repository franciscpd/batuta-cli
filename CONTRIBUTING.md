# Contributing

Use Conventional Commits (for example, `feat: add transcript parser` or
`fix: preserve stream fences`). Changes reach `main` only through pull
requests.

The workspace supports Rust 1.88 and newer; use the pinned stable toolchain in
`rust-toolchain.toml`. Before opening a pull request, run:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
scripts/check-boundaries.sh
```

Crate boundaries are part of the design:

- `compozy-client` is a generic daemon client. It must not depend on
  `ratatui`, `crossterm`, `tui-*`, or `clap`, and its source must not contain
  batuta-specific names.
- `batuta-tui/src/views/` renders from the model only. It must not use
  `compozy_client`, `tokio`, `std::fs`, `std::net`, or `std::process`, even in
  test-only modules.

Contract tests never run from a checkout containing `.compozy/`. Use a
disposable detached worktree and a temporary `COMPOZY_HOME` instead.
