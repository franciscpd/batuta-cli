# batuta-cli

batuta-cli is a batuta-first CompozyOS terminal UI. It gives terminal
operators one keyboard-driven view of sessions, `batuta-deliver` loop runs,
attention items, transcripts, approvals, clarifications, and logs while the
local CompozyOS daemon remains the source of truth.

Status: beta.

# Install

Install from this repository with Cargo:

```console
cargo install --path crates/batuta
```

The local CompozyOS daemon must be available through its Unix socket or local
TCP listener.

# Usage

Run the full application in a terminal:

```console
batuta
```

The delivery-1 command surfaces remain available:

```console
batuta doctor
batuta sessions
batuta tail
```

Use `batuta doctor --json` and `batuta sessions --json` for scripting.

# Releases

Release binaries and checksum files are built for Linux and macOS after a
human merges the standing release PR. Maintainers should follow
[`docs/releasing.md`](docs/releasing.md), including the required PAT setup.

# Config

The optional user config is `$XDG_CONFIG_HOME/batuta/config.toml`, falling
back to `~/.config/batuta/config.toml`:

```toml
# ~/.config/batuta/config.toml
[preset]
agent = "batuta"          # sessions filter and `n` new-session agent
loop = "batuta-deliver"   # runs filter (`?loop=`)
provider = "claude"       # runtime.provider sent with the first prompt of a session created by `n`
model = ""                # runtime.model; empty = provider default

[daemon]
transport = "auto"        # auto | uds | tcp   (flag --daemon wins)
tcp_addr = "localhost:2123"

[ui]
theme = "auto"            # auto | dark | light (uses COLORFGBG when auto)
color = "auto"            # auto | never       (NO_COLOR forces never)
fps = 30                  # render tick, clamped 5..60
sessions_limit = 50       # rows requested for [1] (1..100)
runs_limit = 50           # rows requested for [2]
```

Precedence is flags, environment, file, then defaults. `doctor` reports the
resolved config path and whether it was loaded.

# Keys

| Context | Key | Action |
| --- | --- | --- |
| Global | `1` `2` `3` `4` | focus panel |
| Global | `Tab` / `Shift+Tab` | next / previous panel |
| Global | `w` | workspace picker |
| Global | `L` | logs overlay for the focused item |
| Global | `?` (`F1` in text fields) | help |
| Global | `q` (outside text fields), `Ctrl+C` | quit (draft guard) |
| Lists | `j`/`↓`, `k`/`↑`, `PgUp`, `PgDn`, `g`, `G` | move / jump |
| Lists | `Enter` | open in `[4]` |
| Lists | `/` | filter (type, `Enter` keep, `Esc` clear) |
| Lists | `*` | toggle preset filter (all agents / all loops) |
| Lists | `r` | refresh now |
| Sessions | `n` | new session with preset agent |
| Attention | `a` `x` `A` `X` | permission verbs (task items: `a` approve, `x` reject) |
| Attention | `r` | retry (failure items) |
| Attention | `Enter` | answer clarification / open context |
| Session detail | `j` `k` `PgUp` `PgDn` `g` `G` `Enter` `t` | as `tail` |
| Session detail | `a` `x` `A` `X` | verbs on the selected permission card |
| Session detail | `i` or `Enter` on the composer line | focus composer |
| Session detail | `Ctrl+X` | cancel current turn (confirm) |
| Composer | `Enter` | send (chooser when busy) |
| Composer | `Alt+Enter` (`Shift+Enter` when reported) | newline |
| Composer | `Ctrl+U` | clear |
| Composer | `Esc` | back to transcript |
| Run detail | `p` `u` `k` | pause / resume / kill (confirm) |
| Run detail | `a` `x` | approve / reject the selected gate |
| Run detail | `Enter` | open child run / node session |
| Overlays | `Esc` | close |
| Logs | `e` | toggle error-only |
| Chooser | `j` `k` `1`–`9` `Enter` `Esc` | select / confirm / cancel |

# Contributing

Read the design and developer-experience contracts in
[`docs/internal/specs/2026-08-17-batuta-cli-design.md`](docs/internal/specs/2026-08-17-batuta-cli-design.md)
and [`CONTRIBUTING.md`](CONTRIBUTING.md) before changing the application. Run
the workspace checks relevant to your change; changes reach `main` through
pull requests.
