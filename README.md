# batuta-cli

batuta-cli is a batuta-first CompozyOS terminal UI. It gives terminal
operators one keyboard-driven view of sessions, `batuta-deliver` loop runs,
attention items, transcripts, approvals, clarifications, and logs while the
local CompozyOS daemon remains the source of truth.

Status: beta.

# Install

Prebuilt binaries (Linux x86_64, macOS x86_64/arm64) are attached to each
[GitHub Release](https://github.com/franciscpd/batuta-cli/releases), with
SHA-256 checksum sidecars. While releases are betas, pick the newest tag from
the releases page:

```console
# replace v0.1.0-beta.5 with the newest tag from the releases page
curl -LO https://github.com/franciscpd/batuta-cli/releases/download/v0.1.0-beta.5/batuta-x86_64-unknown-linux-gnu.tar.xz
curl -LO https://github.com/franciscpd/batuta-cli/releases/download/v0.1.0-beta.5/batuta-x86_64-unknown-linux-gnu.tar.xz.sha256
sha256sum -c batuta-x86_64-unknown-linux-gnu.tar.xz.sha256
tar -xf batuta-x86_64-unknown-linux-gnu.tar.xz
```

Or build from source:

```console
cargo install --git https://github.com/franciscpd/batuta-cli batuta
# or, from a checkout:
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
color_depth = "auto"      # auto (truecolor when COLORTERM says so) | ansi16
fps = 30                  # render tick, clamped 5..60
sessions_limit = 50       # rows requested for [1] (1..100)
runs_limit = 50           # rows requested for [2]
notify = true             # bell + OSC 9 when unfocused and attention arrives

[keys]                    # optional remaps — see "Keys" below
# quit = "x"
# filter = ["/", "ctrl+f"]
```

Precedence is flags, environment, file, then defaults. `doctor` reports the
resolved config path and whether it was loaded.

## `[keys]`

Global and Lists keybindings (the two tables below) can be remapped via an
optional `[keys]` table. Other contexts (Attention, Session detail,
Composer, Run detail, Overlays, Logs, Chooser) are not remappable — they
carry text input and modal semantics where remapping risks lockout.

Each entry is `action_name = "combo"` or `action_name = ["combo", ...]`
for multiple combos bound to the same action:

```toml
[keys]
quit = "x"                 # single combo
filter = ["/", "ctrl+f"]   # multiple combos
```

A combo spec is `[modifier+]key`, where `modifier` is one of `ctrl`,
`alt`, `shift` (repeatable, e.g. `"ctrl+shift+p"`), and `key` is either a
single character (case-sensitive — `"L"` and `"l"` are distinct combos) or
one of: `tab`, `esc`/`escape`, `enter`/`return`, `space`, `backspace`,
`up`, `down`, `left`, `right`, `home`, `end`, `pgup`/`pageup`,
`pgdn`/`pagedown`, `f1`–`f12`. `"shift+tab"` is accepted and normalizes to
the same combo as a bare `"tab"` reported with a shift modifier.

Rebinding an action to a combo that's already used by another action
steals it from that action; if that action is left with no combos at all,
config loading warns `action <name> left unbound` (`doctor` and startup
warnings surface this the same way as other config warnings). An unknown
action name or an unparsable combo spec also warns and is skipped, rather
than failing config load.

Action names (Global): `focus_sessions`, `focus_runs`, `focus_attention`,
`focus_detail`, `next_panel`, `previous_panel`, `workspace`, `logs`,
`help`, `quit`, `palette` (`Ctrl+P` by default — opens the command
palette).

Action names (Lists): `move_down`, `move_up`, `page_down`, `page_up`,
`top`, `bottom`, `open`, `filter`, `toggle_scope`, `refresh`, `yank`,
`new_session` (Sessions panel only, regardless of remap).

`Ctrl+C` always quits (with the same draft-loss guard as `q`) and cannot
be remapped or unbound — it stays available even if `quit` is misconfigured.

Combos match modifiers exactly (this is the configurable-keymap contract,
not an accident): binding `filter` to `/` matches a plain `/` only, so an
`Alt`- or `Ctrl`-decorated variant of a combo no longer falls through to
the plain action the way earlier, hardcoded key handling sometimes did.

# Keys

These are the defaults. Global and Lists rows are configurable via the
`[keys]` config table documented above.

| Context | Key | Action |
| --- | --- | --- |
| Global | `1` `2` `3` `4` | focus panel |
| Global | `Tab` / `Shift+Tab` | next / previous panel |
| Global | `w` | workspace picker |
| Global | `L` | logs overlay for the focused item |
| Global | `?` (`F1` in text fields) | help |
| Global | `q` (outside text fields), `Ctrl+C` | quit (draft guard) |
| Global | `Ctrl+P` | command palette |
| Lists | `j`/`↓`, `k`/`↑`, `PgUp`, `PgDn`, `g`, `G` | move / jump |
| Lists | `Enter` | open in `[4]` |
| Lists | `/` | filter (type, `Enter` keep, `Esc` clear) |
| Lists | `*` | toggle preset filter (all agents / all loops) |
| Lists | `r` | refresh now |
| Lists | `y` | yank selected id |
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
