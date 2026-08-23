# UX Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Seven independent UX improvements: install docs, background notifications, OSC 52 yank, transcript search, truecolor opt-in, configurable keybindings, and a command palette.

**Architecture:** Every feature follows the existing Elm shape: state in `Model`, pure `update()` emitting `Cmd` effects, side effects only in `runtime/`, views render from `Model` only. New terminal escapes (bell, OSC 9, OSC 52) become new `Cmd` variants executed in the runtime with unit-testable byte builders.

**Tech Stack:** Rust (edition 2024), ratatui, crossterm 0.29, toml/serde. One new dependency: `base64 = "0.22"` (Task 3 only).

**Spec:** `docs/internal/specs/2026-08-17-batuta-cli-design.md` (this plan extends it; it does not contradict it).

## Global Constraints

- `crates/compozy-client` never depends on `ratatui` and contains no batuta-specific names.
- `crates/batuta-tui/src/views/` renders from `Model` only: no I/O, no `compozy_client` imports (except types re-exported through `app`/`transcript` as today).
- 16-color ANSI palette remains the default; `NO_COLOR` always wins; state never shown by color alone.
- Never run tests as `cargo test --workspace` from this checkout — it contains `.compozy/` and contract tests are forbidden here. Use `cargo test -p batuta-tui` and `cargo test -p batuta` only.
- Unit tests follow the existing `ut_NNN_description` naming; pick unused numbers (grep first: `grep -rho 'ut_[0-9]*' crates | sort -u | tail`).
- Conventional commits; changes reach `main` only through pull requests. Work on a feature branch per task (or one branch for the whole plan if executed inline).
- Config precedence stays: flags > environment > file > defaults; invalid values warn and fall back, never crash (see `crates/batuta/src/config.rs` clamp/warning pattern).

## Task order

Tasks 1–5 are independent. Task 7 (palette) depends on Task 6 (Action enum). Suggested order: 1, 2, 3, 4, 5, 6, 7.

---

### Task 1: README install & release documentation

**Files:**
- Modify: `README.md` (Install section, lines ~10–20)

**Interfaces:**
- Consumes: nothing.
- Produces: docs only.

Context: cargo-dist (`dist-workspace.toml`) already publishes GitHub Release archives + SHA-256 sidecars for `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, but README only documents `cargo install --path`.

- [ ] **Step 1: Verify current release asset names**

Run: `gh release view --repo franciscpd/batuta-cli --json assets --jq '.assets[].name'`
Expected: archives like `batuta-x86_64-unknown-linux-gnu.tar.xz` (+ `.sha256`). Use the actual names in the next step.

- [ ] **Step 2: Rewrite the Install section**

Replace the current Install section body with (keep heading levels; adjust asset names to Step 1 output):

```markdown
# Install

Prebuilt binaries (Linux x86_64, macOS x86_64/arm64) are attached to each
[GitHub Release](https://github.com/franciscpd/batuta-cli/releases), with
SHA-256 checksum sidecars:

```console
curl -LO https://github.com/franciscpd/batuta-cli/releases/latest/download/batuta-x86_64-unknown-linux-gnu.tar.xz
curl -LO https://github.com/franciscpd/batuta-cli/releases/latest/download/batuta-x86_64-unknown-linux-gnu.tar.xz.sha256
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
```

- [ ] **Step 3: Verify the source-install command works syntactically**

Run: `cargo install --path crates/batuta --dry-run 2>&1 | head -3` (from repo root)
Expected: resolves the `batuta` package (any output that is not "package not found").

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: document prebuilt release install"
```

---

### Task 2: Background notifications (bell + OSC 9) on attention

**Files:**
- Modify: `crates/batuta-tui/src/msg.rs` (add `Msg::TerminalFocus(bool)`)
- Modify: `crates/batuta-tui/src/cmd.rs` (add `Cmd::Notify { body: String }`)
- Modify: `crates/batuta-tui/src/app/model.rs` (add `terminal_focused: bool` to `Model`, `notify: bool` to `UiSettings`)
- Modify: `crates/batuta-tui/src/app/update/mod.rs` (handle `Msg::TerminalFocus`)
- Modify: `crates/batuta-tui/src/app/update/attention.rs` (emit `Cmd::Notify` on growth)
- Modify: `crates/batuta-tui/src/runtime/mod.rs` (execute `Cmd::Notify`; map crossterm focus events; enable focus change reporting)
- Modify: `crates/batuta/src/config.rs` + `crates/batuta/src/app.rs` (plumb `[ui] notify`)
- Modify: `README.md` (config docs)
- Test: `crates/batuta-tui/tests/panels_attention.rs` (or a new `notify.rs`), unit tests inline

**Interfaces:**
- Consumes: `Model.attention: Vec<AttentionItem>` rebuild sites in `app/update/attention.rs`.
- Produces: `Cmd::Notify { body: String }`; `Model.terminal_focused: bool` (default `true`); `UiSettings.notify: bool` (default `true`); `pub fn notify_bytes(body: &str) -> Vec<u8>` in `runtime/mod.rs`.

- [ ] **Step 1: Write failing unit test for the escape bytes** (in `runtime/mod.rs` tests module)

```rust
#[test]
fn ut_760_notify_bytes_emit_bell_and_osc9() {
    let bytes = super::notify_bytes("2 attention items");
    assert!(bytes.starts_with(b"\x07"));
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("\x1b]9;2 attention items\x1b\\"));
}
```

Run: `cargo test -p batuta-tui ut_760` — Expected: FAIL (function not defined).

- [ ] **Step 2: Implement `notify_bytes` and the `Cmd::Notify` arm**

In `runtime/mod.rs`:

```rust
/// BEL (legacy attention) followed by OSC 9 (desktop notification on
/// supporting terminals: kitty, wezterm, foot, iTerm2). ST terminator.
pub fn notify_bytes(body: &str) -> Vec<u8> {
    let mut bytes = vec![0x07];
    bytes.extend_from_slice(b"\x1b]9;");
    // strip control chars so a transcript-derived body cannot break the sequence
    bytes.extend(body.chars().filter(|c| !c.is_control()).collect::<String>().into_bytes());
    bytes.extend_from_slice(b"\x1b\\");
    bytes
}
```

In `execute()`'s match, alongside `Cmd::Render`:

```rust
Cmd::Notify { body } => {
    use std::io::Write;
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(&notify_bytes(&body));
    let _ = stdout.flush();
}
```

Add to `cmd.rs`:

```rust
pub enum Cmd {
    // ... existing variants ...
    Notify { body: String },
}
```

Run: `cargo test -p batuta-tui ut_760` — Expected: PASS.

- [ ] **Step 3: Write failing update-level test** (new file `crates/batuta-tui/tests/notify.rs`, mirroring the builder helpers used in `tests/panels_attention.rs` — copy its `Model` setup)

```rust
// Pseudocode shape — reuse the existing attention fixture helpers from
// tests/panels_attention.rs to deliver an overview/permission payload.
#[test]
fn notify_fires_only_when_unfocused_and_enabled() {
    let mut model = model_with_workspace();          // helper from existing tests
    model.terminal_focused = false;
    let cmds = deliver_attention_item(&mut model);   // Msg::Api overview with 1 item
    assert!(cmds.iter().any(|c| matches!(c, Cmd::Notify { .. })));

    let mut focused = model_with_workspace();
    focused.terminal_focused = true;
    let cmds = deliver_attention_item(&mut focused);
    assert!(!cmds.iter().any(|c| matches!(c, Cmd::Notify { .. })));

    let mut disabled = model_with_workspace();
    disabled.terminal_focused = false;
    disabled.settings.ui.notify = false;
    let cmds = deliver_attention_item(&mut disabled);
    assert!(!cmds.iter().any(|c| matches!(c, Cmd::Notify { .. })));
}
```

Run: `cargo test -p batuta-tui --test notify` — Expected: FAIL (fields missing).

- [ ] **Step 4: Add state and emission logic**

`model.rs`: add `pub terminal_focused: bool` to `Model` (init `true` in the constructor), `pub notify: bool` to `UiSettings` (`Default` → `true`).

`app/update/mod.rs`: handle the new message:

```rust
Msg::TerminalFocus(focused) => {
    model.terminal_focused = focused;
    Vec::new()
}
```

`app/update/attention.rs`: locate where `model.attention` is rebuilt (grep `model.attention =`). Capture `let before = model.attention.len();` above the rebuild and append after it:

```rust
let after = model.attention.len();
if after > before && !model.terminal_focused && model.settings.ui.notify {
    commands.push(Cmd::Notify {
        body: format!("batuta: {after} attention item{}", if after == 1 { "" } else { "s" }),
    });
}
```

If the rebuild has several call sites, extract this into `fn notify_on_growth(model: &Model, before: usize, commands: &mut Vec<Cmd>)` in the same module and call it from each.

Run: `cargo test -p batuta-tui --test notify` — Expected: PASS.

- [ ] **Step 5: Wire crossterm focus events**

`msg.rs`: add `TerminalFocus(bool)` to `Msg`.

`runtime/mod.rs` event loop (near line 474 where `Event::Mouse` is mapped):

```rust
Ok(Event::FocusGained) => Msg::TerminalFocus(true),
Ok(Event::FocusLost) => Msg::TerminalFocus(false),
```

Enable reporting where the terminal enters raw mode (follow how `ratatui::init` is invoked in `crates/batuta/src/app.rs`; add right after init and mirror in restore):

```rust
let _ = crossterm::execute!(std::io::stdout(), crossterm::event::EnableFocusChange);
// on restore path (TerminalGuard / RatatuiOps::restore in crates/batuta/src/terminal.rs):
let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableFocusChange);
```

Terminals that never report focus leave `terminal_focused == true`, so behavior degrades to "no notifications" — safe default.

- [ ] **Step 6: Plumb config**

`crates/batuta/src/config.rs`: add `pub notify: Option<bool>` to `UiFile`; in `resolve()` map it into `UiSettings.notify` (default `true`). Add unit test alongside `ut_631`:

```rust
#[test]
fn ut_761_notify_key_resolves_and_defaults_true() {
    // extend the existing full-file fixture with `notify = false` and assert
    // settings.ui.notify == false; assert the missing-file default is true.
}
```

README config block: add `notify = true             # bell + OSC 9 when unfocused and attention arrives`.

- [ ] **Step 7: Full test pass and commit**

Run: `cargo test -p batuta-tui && cargo test -p batuta` — Expected: PASS.

```bash
git add -A && git commit -m "feat(tui): notify on attention when terminal unfocused"
```

---

### Task 3: Yank to clipboard via OSC 52

**Files:**
- Modify: `Cargo.toml` (workspace) + `crates/batuta-tui/Cargo.toml` (add `base64 = "0.22"`)
- Modify: `crates/batuta-tui/src/cmd.rs` (add `Cmd::CopyToClipboard(String)`)
- Modify: `crates/batuta-tui/src/runtime/mod.rs` (execute it; `osc52_bytes`)
- Modify: `crates/batuta-tui/src/transcript.rs` (add `entry_plain_text`)
- Modify: `crates/batuta-tui/src/app/update/keys.rs` (`y` in lists and session detail)
- Modify: `crates/batuta-tui/src/keymap.rs` (document `y`)
- Test: unit tests inline + `crates/batuta-tui/tests/detail_session.rs`

**Interfaces:**
- Consumes: `TranscriptState::entries()`, `presentation_rows()`, `ListState::selected()`, `Toast`/`ToastKind` (existing).
- Produces: `Cmd::CopyToClipboard(String)`; `pub fn osc52_bytes(payload: &str) -> Vec<u8>` (runtime); `pub fn entry_plain_text(entry: &Entry) -> String` (transcript.rs).

- [ ] **Step 1: Failing test for OSC 52 bytes** (runtime tests)

```rust
#[test]
fn ut_762_osc52_bytes_wrap_base64_payload() {
    let bytes = super::osc52_bytes("hello");
    let text = String::from_utf8(bytes).unwrap();
    assert_eq!(text, "\x1b]52;c;aGVsbG8=\x1b\\");
}
```

Run: `cargo test -p batuta-tui ut_762` — Expected: FAIL.

- [ ] **Step 2: Implement**

Add `base64.workspace = true` (declare `base64 = "0.22"` in the workspace `[workspace.dependencies]`).

```rust
pub fn osc52_bytes(payload: &str) -> Vec<u8> {
    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD.encode(payload.as_bytes());
    format!("\x1b]52;c;{encoded}\x1b\\").into_bytes()
}
```

`Cmd::CopyToClipboard(String)` arm in `execute()`, same stdout write/flush pattern as `Cmd::Notify`. Cap the payload: OSC 52 receivers commonly truncate near 100 KB — `payload.chars().take(64 * 1024)`.

Run: `cargo test -p batuta-tui ut_762` — Expected: PASS.

- [ ] **Step 3: Failing test for `entry_plain_text`** (transcript.rs tests; reuse the local `entry()` helper)

```rust
#[test]
fn ut_763_entry_plain_text_extracts_text_and_tool_parts() {
    let text_entry = entry(1, Part::Text { text: "hello *world*".into() });
    assert_eq!(entry_plain_text(&text_entry), "hello *world*");
    let tool = entry(2, Part::Tool {
        name: "bash".into(), tool_call_id: None, state: Some("completed".into()),
        input: None, output: Some(serde_json::json!("ls -la")), error_text: None, title: None,
    });
    let text = entry_plain_text(&tool);
    assert!(text.contains("bash"));
    assert!(text.contains("ls -la"));
}
```

(Adjust `Part::Text` / field names to the actual `compozy_client::types::Part` variants — check `crates/compozy-client/src/transcript.rs` or `types` before writing.)

Run: `cargo test -p batuta-tui ut_763` — Expected: FAIL.

- [ ] **Step 4: Implement `entry_plain_text`** (transcript.rs)

```rust
/// Lossless-enough plain text for clipboard: text parts verbatim; tool parts
/// as "name\ninput\noutput"; everything else via its JSON value.
pub fn entry_plain_text(entry: &Entry) -> String {
    entry.message.parts.iter().map(|part| match part {
        Part::Text { text } => text.clone(),
        Part::Tool { name, input, output, error_text, .. } => {
            let mut s = name.clone();
            for value in [input, output].into_iter().flatten() {
                s.push('\n');
                s.push_str(&value.as_str().map(str::to_owned)
                    .unwrap_or_else(|| value.to_string()));
            }
            if let Some(err) = error_text { s.push('\n'); s.push_str(err); }
            s
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    }).collect::<Vec<_>>().join("\n")
}
```

(Match arm names against the real `Part` enum; `Reasoning`, `Event`, `Data` variants go through the `other` arm.)

Run: `cargo test -p batuta-tui ut_763` — Expected: PASS.

- [ ] **Step 5: Bind `y`**

`keys.rs` `list_key()` (Sessions/Runs focus): on `KeyCode::Char('y')`, copy the selected row id and toast:

```rust
KeyCode::Char('y') => {
    let copied = match model.focus {
        Panel::Sessions => model.sessions.selected().map(|row| row.id.clone()),
        Panel::Runs => model.runs.selected().map(|row| row.id.clone()),
        _ => None,
    };
    if let Some(text) = copied {
        model.toast = Some(Toast { kind: ToastKind::Info, text: format!("copied {text}"), sticky: false });
        return vec![Cmd::CopyToClipboard(text), Cmd::After(Duration::from_secs(4), TimerId::ToastExpiry)];
    }
    Vec::new()
}
```

(Copy the exact toast+timer idiom already used elsewhere in `keys.rs`/`api.rs` — grep `ToastExpiry` — instead of inventing a new one.)

`detail_key()` (SessionDetail, not composer-focused): `y` resolves the selected presentation row, joins `entry_plain_text` over its entries, emits `Cmd::CopyToClipboard` + "copied entry" toast.

Add to `keymap.rs` BINDINGS: `Lists: "y" → "yank id"`, `SessionDetail`: extend the navigate binding row with `y`.

- [ ] **Step 6: Update-level test** (`tests/detail_session.rs`): press `y` on a selected entry, assert a `Cmd::CopyToClipboard(text)` where `text` contains the entry text, and a toast is set.

Run: `cargo test -p batuta-tui --test detail_session` — Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat(tui): yank ids and transcript entries via OSC 52"
```

---

### Task 4: Transcript search (`/`, `n`/`N`)

**Files:**
- Modify: `crates/batuta-tui/src/app/model.rs` (add `SearchState`; field on `TranscriptView`)
- Modify: `crates/batuta-tui/src/app/update/keys.rs` (`/` opens, char input, `Enter`/`Esc`, `n`/`N`)
- Modify: `crates/batuta-tui/src/app/update/stream.rs` (recompute matches on transcript change)
- Modify: `crates/batuta-tui/src/views/session.rs` (footer shows search status)
- Modify: `crates/batuta-tui/src/keymap.rs` (bindings + help)
- Test: `crates/batuta-tui/tests/detail_session.rs` additions

**Interfaces:**
- Consumes: `entry_plain_text` from Task 3 (if Task 3 is skipped, implement that helper here — it is self-contained).
- Produces: on `TranscriptView`: `pub search: Option<SearchState>` with

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchState {
    pub query: String,
    pub focused: bool,        // typing in the search prompt
    pub matches: Vec<usize>,  // presentation-row indexes
    pub current: usize,       // index into matches
}
```

and `pub fn recompute_search(detail: &mut SessionDetail)` in `app/update/keys.rs` (or a small `app/update/search.rs`).

Behavior contract:
- `/` in SessionDetail (composer not focused, no overlay) opens the prompt (`focused: true`), `follow` stays untouched until a jump happens.
- Printable chars append to `query`; `Backspace` pops; matches recompute live; `Esc` clears the whole search; `Enter` confirms (`focused: false`) and jumps to the first match at-or-after the current selection.
- While a confirmed search exists: `n` next match, `N` previous (wrapping); a jump sets `view.selection = matches[current]` and `view.follow = false`.
- Matching: case-insensitive substring of the presentation row's plain text (group rows match on their label plus member entries).
- While `search.focused`, search input wins over other detail keys (mirror how `filter_focused` works for lists — grep `filter_focused` in `keys.rs` and copy the structure).

- [ ] **Step 1: Failing tests** (`tests/detail_session.rs`; use existing helpers to build a session detail with entries "alpha", "beta", "alpha two")

```rust
#[test]
fn search_jumps_and_cycles_matches() {
    let mut model = session_with_entries(&["alpha", "beta", "alpha two"]);
    press(&mut model, '/');
    type_str(&mut model, "alpha");
    press_key(&mut model, KeyCode::Enter);
    let view = &model.session_detail().unwrap().view;
    assert_eq!(view.selection, 0);
    assert!(!view.follow);
    press(&mut model, 'n');
    assert_eq!(model.session_detail().unwrap().view.selection, 2);
    press(&mut model, 'n'); // wraps
    assert_eq!(model.session_detail().unwrap().view.selection, 0);
    press_key(&mut model, KeyCode::Esc);
    assert!(model.session_detail().unwrap().view.search.is_none());
}
```

Run: `cargo test -p batuta-tui --test detail_session search_jumps` — Expected: FAIL.

- [ ] **Step 2: Implement state + key handling** per the contract above. `recompute_search`:

```rust
pub(super) fn recompute_search(detail: &mut SessionDetail) {
    let Some(search) = detail.view.search.as_mut() else { return };
    let needle = search.query.to_lowercase();
    let entries = detail.transcript.entries();
    search.matches = detail.transcript.presentation_rows(detail.view.raw_debug)
        .iter().enumerate()
        .filter(|(_, row)| {
            if needle.is_empty() { return false; }
            let haystack = match row {
                PresentationRow::Entry { entry_index } => entries.get(*entry_index)
                    .map(|e| crate::transcript::entry_plain_text(e)).unwrap_or_default(),
                PresentationRow::Group { entry_indexes, label } => {
                    let mut s = label.clone();
                    for i in entry_indexes { if let Some(e) = entries.get(*i) { s.push('\n'); s.push_str(&crate::transcript::entry_plain_text(e)); } }
                    s
                }
            };
            haystack.to_lowercase().contains(&needle)
        })
        .map(|(index, _)| index)
        .collect();
    search.current = search.current.min(search.matches.len().saturating_sub(1));
}
```

Call it from every site in `stream.rs`/`api.rs` that applies a snapshot, delta, or prepended page to the focused session (grep `apply_snapshot\|apply_delta\|prepend_page` in `app/update/`).

Run: `cargo test -p batuta-tui --test detail_session` — Expected: PASS.

- [ ] **Step 3: Footer + help**

`views/session.rs` `footer()`: prepend search status when active:

```rust
if let Some(search) = &detail.view.search {
    let position = if search.matches.is_empty() { "0/0".into() }
        else { format!("{}/{}", search.current + 1, search.matches.len()) };
    return if search.focused {
        format!("search: {}▏  Enter confirm  Esc cancel", search.query)
    } else {
        format!("search \"{}\" · {position} · n/N next/prev · Esc clear", search.query)
    };
}
```

`keymap.rs`: add `SessionDetail: "/" → "search"` and extend navigate row with `n/N`.

- [ ] **Step 4: Full pass + commit**

Run: `cargo test -p batuta-tui` — Expected: PASS.

```bash
git add -A && git commit -m "feat(tui): search transcript with / and n/N"
```

---

### Task 5: Truecolor opt-in palette

**Files:**
- Modify: `crates/batuta-tui/src/theme.rs` (ColorDepth, RGB palettes)
- Modify: `crates/batuta-tui/src/app/model.rs` (`UiSettings.color_depth`)
- Modify: `crates/batuta/src/config.rs` (`[ui] color_depth`), `crates/batuta/src/app.rs` (COLORTERM detection at the `Theme` construction site — grep `Theme::` in `crates/batuta/src`)
- Modify: `README.md`
- Test: theme.rs unit tests, config tests

**Interfaces:**
- Produces:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorDepth {
    #[default]
    Ansi16,
    TrueColor,
}

impl Theme {
    pub fn with_options(color: bool, variant: ThemeVariant, colorfgbg: Option<&str>, depth: ColorDepth) -> Self;
}
```

`with_variant` keeps its signature and delegates to `with_options(..., ColorDepth::Ansi16)` so every existing caller and test is untouched. `UiSettings` gains `pub color_depth: ColorDepthMode` (`Auto | Ansi16`, default `Auto`); resolution to a concrete `ColorDepth` happens in `crates/batuta` (Auto → TrueColor iff `COLORTERM` is `truecolor` or `24bit`, else Ansi16) — the TUI crate never reads env vars for this, mirroring how `COLORFGBG`/`NO_COLOR` flow in today.

- [ ] **Step 1: Failing tests** (theme.rs)

```rust
#[test]
fn ut_764_ansi16_depth_never_emits_rgb() {
    for variant in [ThemeVariant::Dark, ThemeVariant::Light] {
        let theme = Theme::with_options(true, variant, None, ColorDepth::Ansi16);
        for token in ALL_TOKENS { // reuse the token list from ut_003
            assert!(!matches!(theme.style(token).fg, Some(Color::Rgb(..))));
        }
    }
}

#[test]
fn ut_765_truecolor_depth_uses_rgb_and_respects_no_color() {
    let theme = Theme::with_options(true, ThemeVariant::Dark, None, ColorDepth::TrueColor);
    assert!(matches!(theme.style(SemanticToken::Active).fg, Some(Color::Rgb(..))));
    let plain = Theme::with_options(false, ThemeVariant::Dark, None, ColorDepth::TrueColor);
    assert_eq!(plain.style(SemanticToken::Active).fg, None);
}
```

Run: `cargo test -p batuta-tui ut_764 ut_765` — Expected: FAIL.

- [ ] **Step 2: Implement palettes**

In `with_options`, when `depth == TrueColor`, replace the six slot colors:

```rust
// Dark (default terminal-dark backgrounds):
// muted #8a8f98, active #4fc1e9 (cyan), success #5cb85c, waiting #e0a83f,
// error #e5534b, system #b07fd8, default fg #d8dee9
// Light:
// muted #6c7078, active #1a6fb5 (blue), success #2e7d32, waiting #b07d10,
// error #c62828, system #7b3fa0, default fg unset (terminal default)
```

Express as `Color::Rgb(0x4f, 0xc1, 0xe9)` etc.; keep the existing DIM/BOLD modifier logic identical — only the `Color` values change with depth. State markers (`✓`, `×`, …) are untouched, so the never-color-alone rule holds.

Run: `cargo test -p batuta-tui ut_764 ut_765` and the whole theme module — Expected: PASS (existing `ut_701` still passes because `with_variant` stays Ansi16).

- [ ] **Step 3: Config plumbing**

`UiFile` gains `pub color_depth: Option<String>`; parse `"auto" | "ansi16"` with the same warn-and-fallback pattern as `parse_theme`. Thread the resolved `ColorDepth` into the `Theme` construction site in `crates/batuta/src/app.rs`. Config unit test: `ut_766` asserting file value read, invalid value warns and falls back to `Auto`, and `NO_COLOR` still wins end-to-end.

README config block: `color_depth = "auto"      # auto (truecolor when COLORTERM says so) | ansi16`.

- [ ] **Step 4: Full pass + commit**

Run: `cargo test -p batuta-tui && cargo test -p batuta` — Expected: PASS.

```bash
git add -A && git commit -m "feat(tui): truecolor palette behind color_depth"
```

---

### Task 6: Configurable keybindings (`[keys]`)

Scope (deliberate v1): only `Context::Global` and `Context::Lists` actions are remappable. Other contexts (composer, chooser, overlays) stay hardcoded — they carry text input and modal semantics where remapping risks lockout.

**Files:**
- Modify: `crates/batuta-tui/src/keymap.rs` (Action enum, KeyCombo parser, `Keymap` type, dynamic footer/help)
- Modify: `crates/batuta-tui/src/app/model.rs` (`Settings.keymap: Keymap`)
- Modify: `crates/batuta-tui/src/app/update/keys.rs` (`key()`/`list_key()` consult the keymap first)
- Modify: `crates/batuta/src/config.rs` (`[keys]` table)
- Modify: `README.md`
- Test: keymap.rs unit tests, config tests, `tests/boundaries.rs` untouched

**Interfaces:**
- Produces (in `keymap.rs`):

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    // Global
    FocusSessions, FocusRuns, FocusAttention, FocusDetail,
    NextPanel, PreviousPanel, Workspace, Logs, Help, Quit, Palette,
    // Lists
    MoveDown, MoveUp, PageDown, PageUp, Top, Bottom,
    Open, Filter, ToggleScope, Refresh, Yank, NewSession,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyCombo { pub code: KeyCode, pub modifiers: KeyModifiers }

#[derive(Clone, Debug, PartialEq)]
pub struct Keymap { /* HashMap<KeyCombo, Action> per context group */ }

impl Keymap {
    pub fn action(&self, context_group: ContextGroup, key: &KeyEvent) -> Option<Action>;
    pub fn combos(&self, action: Action) -> Vec<KeyCombo>;      // for footer/help
}
impl Default for Keymap { /* today's BINDINGS */ }

pub fn parse_combo(spec: &str) -> Result<KeyCombo, String>;      // "q", "ctrl+c", "shift+tab", "f1", "pgdn"
pub fn action_by_name(name: &str) -> Option<Action>;             // "quit", "workspace", "move_down", ...
```

where `ContextGroup` is `Global | Lists`. `Action::Palette` exists but is bound to nothing by default until Task 7 binds `ctrl+p`.

- [ ] **Step 1: Failing parser tests**

```rust
#[test]
fn ut_767_parse_combo_accepts_documented_forms() {
    assert_eq!(parse_combo("q").unwrap(), KeyCombo { code: KeyCode::Char('q'), modifiers: KeyModifiers::NONE });
    assert_eq!(parse_combo("ctrl+c").unwrap().modifiers, KeyModifiers::CONTROL);
    assert_eq!(parse_combo("shift+tab").unwrap().code, KeyCode::BackTab);
    assert_eq!(parse_combo("f1").unwrap().code, KeyCode::F(1));
    assert_eq!(parse_combo("pgdn").unwrap().code, KeyCode::PageDown);
    assert!(parse_combo("meh").is_err());
}

#[test]
fn ut_768_default_keymap_matches_documented_bindings() {
    let keymap = Keymap::default();
    let quit = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    assert_eq!(keymap.action(ContextGroup::Global, &quit), Some(Action::Quit));
    let filter = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    assert_eq!(keymap.action(ContextGroup::Lists, &filter), Some(Action::Filter));
}
```

Run: `cargo test -p batuta-tui ut_767 ut_768` — Expected: FAIL.

- [ ] **Step 2: Implement `keymap.rs` types.** Notes: `shift+tab` must produce `KeyCode::BackTab` (crossterm reports it that way); `Char` combos compare case-insensitively on the char but `Shift` for uppercase letters must be tolerated in `action()` (normalize: uppercase char → lowercase char + SHIFT stripped for `Char` codes). Keep `BINDINGS`/`HELP_ORDER` as the display fallback for non-remappable contexts.

Run: `cargo test -p batuta-tui ut_767 ut_768` — Expected: PASS.

- [ ] **Step 3: Route `keys.rs` through the keymap.** At the top of `key()` (before the current Global match) and in `list_key()`:

```rust
if let Some(action) = model.settings.keymap.action(ContextGroup::Global, &key) {
    return apply_global_action(model, action);
}
```

Extract the existing bodies of the global/list match arms into:

```rust
pub(super) fn apply_global_action(model: &mut Model, action: Action) -> Vec<Cmd>;
pub(super) fn apply_list_action(model: &mut Model, action: Action) -> Vec<Cmd>;
```

This is a mechanical move: each old `KeyCode::Char('w') => { ... }` body becomes the `Action::Workspace => { ... }` arm. Do NOT change behavior. Guard rails that must survive: text-input contexts (`filter_focused`, composer, clarify text, search from Task 4) are checked before keymap lookup, exactly where they are checked today; `Ctrl+C` quit must remain reachable even if the user remaps `quit` badly — hardcode `Ctrl+C → guarded_quit` before the lookup.

Run: `cargo test -p batuta-tui` — Expected: PASS (existing key tests are the regression net).

- [ ] **Step 4: Footer/help read the live keymap.** `keymap::footer(&contexts)` and `help_lines()` gain a `&Keymap` parameter; for Global/Lists rows render `keymap.combos(action)` (e.g. `"j/↓"`), for other contexts keep the static strings. Update the two call sites (`views/footer.rs`, `views/overlays.rs` Help arm). Snapshot-ish test: default keymap footer output equals today's string.

- [ ] **Step 5: Config `[keys]`.** In `config.rs`: `ConfigFile` gains `#[serde(default)] pub keys: toml::Table` (or `HashMap<String, toml::Value>`); in `resolve()`:

```rust
for (name, value) in keys_table {
    let Some(action) = keymap::action_by_name(&name) else {
        warnings.push(format!("unknown key action `{name}`")); continue;
    };
    let specs: Vec<String> = match value { /* string or array of strings */ };
    let mut combos = Vec::new();
    for spec in specs {
        match keymap::parse_combo(&spec) {
            Ok(combo) => combos.push(combo),
            Err(err) => warnings.push(format!("key `{name}`: {err}")),
        }
    }
    if !combos.is_empty() { keymap.rebind(action, combos); }
}
```

`rebind` replaces the action's combos and removes collisions (a combo taken from another action leaves that action on its remaining combos; if none remain, warn "action X left unbound"). Config test `ut_769`: file with `quit = "x"` and `filter = ["ctrl+f"]` resolves; unknown action and bad combo produce warnings, not errors. `Settings` (TUI) gains `pub keymap: Keymap` (default in `Default`), plumbed via `tui_settings()`.

README: document the `[keys]` table, the action-name list, and the combo grammar.

- [ ] **Step 6: Full pass + commit**

Run: `cargo test -p batuta-tui && cargo test -p batuta` — Expected: PASS.

```bash
git add -A && git commit -m "feat(tui): configurable global and list keybindings"
```

---

### Task 7: Command palette (`Ctrl+P`)

Depends on Task 6 (`Action`, `apply_global_action`, `apply_list_action`).

**Files:**
- Modify: `crates/batuta-tui/src/app/model.rs` (Overlay variant)
- Create: `crates/batuta-tui/src/app/update/palette.rs`
- Modify: `crates/batuta-tui/src/app/update/keys.rs` (open; route keys when open)
- Modify: `crates/batuta-tui/src/app/update/mod.rs` (module decl)
- Modify: `crates/batuta-tui/src/views/overlays.rs` (render)
- Modify: `crates/batuta-tui/src/keymap.rs` (bind `ctrl+p` → `Action::Palette` in `Keymap::default`; help entry)
- Test: new `crates/batuta-tui/tests/palette.rs`

**Interfaces:**
- Consumes: `Action`, `apply_global_action`, `apply_list_action` (Task 6), `Overlay`, `centered` (overlays.rs).
- Produces:

```rust
// model.rs
Overlay::Palette { query: String, selected: usize }

// app/update/palette.rs
pub struct PaletteEntry { pub label: &'static str, pub action: Action }
pub fn entries(model: &Model, query: &str) -> Vec<PaletteEntry>;
pub fn key(model: &mut Model, key: KeyEvent) -> Vec<Cmd>;
```

Catalog (static list, filtered case-insensitively by substring on label):

```rust
const CATALOG: &[PaletteEntry] = &[
    PaletteEntry { label: "focus: sessions", action: Action::FocusSessions },
    PaletteEntry { label: "focus: deliver runs", action: Action::FocusRuns },
    PaletteEntry { label: "focus: attention", action: Action::FocusAttention },
    PaletteEntry { label: "focus: detail", action: Action::FocusDetail },
    PaletteEntry { label: "workspace: switch", action: Action::Workspace },
    PaletteEntry { label: "logs: open overlay", action: Action::Logs },
    PaletteEntry { label: "list: toggle all agents/loops", action: Action::ToggleScope },
    PaletteEntry { label: "list: refresh now", action: Action::Refresh },
    PaletteEntry { label: "session: new", action: Action::NewSession },
    PaletteEntry { label: "help", action: Action::Help },
    PaletteEntry { label: "quit", action: Action::Quit },
];
```

Behavior: `Ctrl+P` (any non-text context) opens with empty query. Typing filters; `j`/`k`/arrows move; `Enter` closes the overlay first, then dispatches: global actions via `apply_global_action`; list actions via `apply_list_action` (which act on the currently focused list — for `NewSession` focus Sessions first). `Esc` closes. Entries whose action cannot apply in the current state (e.g. `NewSession` while onboarding overlay logic pending) simply no-op exactly as their key equivalent does today — the palette adds no new state rules.

- [ ] **Step 1: Failing tests** (`tests/palette.rs`, reusing model helpers from `tests/picker.rs`)

```rust
#[test]
fn palette_opens_filters_and_dispatches() {
    let mut model = model_with_workspace();
    press_ctrl(&mut model, 'p');
    assert!(matches!(model.overlay, Some(Overlay::Palette { .. })));
    type_str(&mut model, "quit");
    press_key(&mut model, KeyCode::Enter);
    // quit is guarded: first Enter arms the guard exactly like pressing q
    assert!(model.overlay.is_none());
}

#[test]
fn palette_esc_closes_without_dispatch() {
    let mut model = model_with_workspace();
    press_ctrl(&mut model, 'p');
    press_key(&mut model, KeyCode::Esc);
    assert!(model.overlay.is_none());
    assert_eq!(model.focus, Panel::Sessions);
}
```

Run: `cargo test -p batuta-tui --test palette` — Expected: FAIL.

- [ ] **Step 2: Implement** state, `palette::key`, opening via `Action::Palette` arm in `apply_global_action`, and routing in `overlay_key()` (add a `Overlay::Palette` branch delegating to `palette::key` before the generic `Esc` handling).

Run: `cargo test -p batuta-tui --test palette` — Expected: PASS.

- [ ] **Step 3: Render** in `views/overlays.rs`, before the final generic match (same early-return pattern as picker):

```rust
if let Some(Overlay::Palette { query, selected }) = &model.overlay {
    let area = centered(frame.area(), 60, 16);
    let entries = crate::app::update::palette::entries(model, query);
    let mut lines = vec![Line::from(format!("> {query}▏"))];
    for (index, entry) in entries.iter().enumerate() {
        let line = Line::from(format!("  {}", entry.label));
        lines.push(if index == *selected { line.style(model.theme.selection) } else { line });
    }
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(Block::default().title("palette").borders(Borders::ALL))
            .style(model.theme.default),
        area,
    );
    return;
}
```

Note: this requires `palette::entries` to be reachable from views without importing `compozy_client` — it lives in `app::update::palette` and takes `&Model`, which satisfies the boundary (views already import from `crate::app`). If the boundary test flags `views` importing from `app::update`, move `entries` + `PaletteEntry` into `model.rs` instead.

- [ ] **Step 4: Bind and document.** `Keymap::default()` binds `ctrl+p` → `Action::Palette`; keymap help gains `Global: "Ctrl+P" → "palette"`; README key table row.

- [ ] **Step 5: Full pass + commit**

Run: `cargo test -p batuta-tui` — Expected: PASS.

```bash
git add -A && git commit -m "feat(tui): command palette on ctrl+p"
```

---

## Out of scope / already done

- Off-tail "new below" pill: already shipped (`FooterState::NewBelow` → "N new below — G to jump").
- Relative timestamps in lists: already shipped (`sessions::relative` in panels [1] and [2]).
- Tag/changelog housekeeping: resolved by `git fetch --tags` (beta.4/5 present); keep local `main` synced with `git pull`.
- Mouse support beyond scroll: explicit spec non-goal; unchanged.

## Verification checklist per task

Each task ends with `cargo test -p batuta-tui && cargo test -p batuta` green plus, for visual changes (Tasks 4, 5, 7), a manual run against a live daemon (`batuta`) before the PR. Never run contract tests from this checkout.
