# batuta-cli

Rust/Ratatui terminal UI for CompozyOS, batuta-first. Design lives in
`docs/internal/specs/2026-08-17-batuta-cli-design.md`; read it before any
architectural change. Plans go in `docs/internal/plans/`.

- `crates/compozy-client` never depends on `ratatui` and never contains
  batuta-specific names — it is the generic daemon client.
- `crates/batuta-tui/src/views/` renders from `Model` only: no I/O, no
  `compozy_client` imports.
- Quitting the TUI never stops or cancels a session or loop run.
- Never run contract tests from a checkout containing `.compozy/`; use a
  disposable detached worktree with a temporary `COMPOZY_HOME`.
- Changes reach `main` only through pull requests.
