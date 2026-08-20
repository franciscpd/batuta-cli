---
round: 1
round_created_at: 2026-08-20T20:10:59.932630868Z
status: valid
file: crates/batuta/src/tail.rs
line: 98
severity: high
author: unknown
---

# Issue 001: Tail ignores configured theme and NO_COLOR

## Review Comment

`batuta tail` builds its model through `Model::tail`, which creates the theme from default settings. It then overwrites only `model.settings.ui` at lines98–99, after the theme was already created in `Model::new` (model.rs:532–536). Consequently `[ui] theme = "light"`, `[ui] color = "never"`, and `NO_COLOR` do not affect tail mode; it continues rendering with the default colored auto theme. This violates the semantic-theme and NO_COLOR requirements for a TUI surface. Construct the tail model with the supplied UI settings (or recompute the theme and invalidate its render cache after assigning settings), and add a tail-mode configuration test.

## Triage

- Decision: `VALID`
- Root cause: `Model::tail` builds `Theme` from default `Settings`, while tail then replaced
  only `model.settings.ui`; no subsequent code refreshed the derived theme.
- Fix: tail now applies its preset and UI settings together, rebuilds the theme from the resolved
  color and theme modes, and invalidates the transcript render cache. `ut_106` covers light theme
  selection and `ColorMode::Never`, the value produced for `NO_COLOR`.
