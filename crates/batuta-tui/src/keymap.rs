use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Context {
    Global,
    Lists,
    Sessions,
    Attention,
    SessionDetail,
    Composer,
    RunDetail,
    Overlays,
    Logs,
    Chooser,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub context: Context,
    pub keys: &'static str,
    pub action: &'static str,
}

pub const BINDINGS: &[Binding] = &[
    Binding {
        context: Context::Global,
        keys: "1-4",
        action: "focus",
    },
    Binding {
        context: Context::Global,
        keys: "Tab/Shift+Tab",
        action: "next/previous",
    },
    Binding {
        context: Context::Global,
        keys: "w",
        action: "workspace",
    },
    Binding {
        context: Context::Global,
        keys: "L",
        action: "logs",
    },
    Binding {
        context: Context::Global,
        keys: "?/F1",
        action: "help",
    },
    Binding {
        context: Context::Global,
        keys: "q/Ctrl+C",
        action: "quit",
    },
    Binding {
        context: Context::Global,
        keys: "Ctrl+P",
        action: "palette",
    },
    Binding {
        context: Context::Lists,
        keys: "j/k/↑/↓/PgUp/PgDn/g/G",
        action: "move",
    },
    Binding {
        context: Context::Lists,
        keys: "Enter",
        action: "open",
    },
    Binding {
        context: Context::Lists,
        keys: "/",
        action: "filter",
    },
    Binding {
        context: Context::Lists,
        keys: "*/r",
        action: "preset/refresh",
    },
    Binding {
        context: Context::Lists,
        keys: "y",
        action: "yank id",
    },
    Binding {
        context: Context::Sessions,
        keys: "n",
        action: "new session",
    },
    Binding {
        context: Context::Attention,
        keys: "a/A/x/X/r/Enter/o",
        action: "respond/open",
    },
    Binding {
        context: Context::SessionDetail,
        keys: "j/k/PgUp/PgDn/g/G/Enter/t/y/n/N",
        action: "navigate",
    },
    Binding {
        context: Context::SessionDetail,
        keys: "i/Ctrl+X",
        action: "compose/cancel",
    },
    Binding {
        context: Context::SessionDetail,
        keys: "/",
        action: "search",
    },
    Binding {
        context: Context::Composer,
        keys: "Enter/Alt+Enter/Ctrl+U/Esc",
        action: "send/newline/clear/back",
    },
    Binding {
        context: Context::RunDetail,
        keys: "p/u/k/a/x/Enter",
        action: "control/open",
    },
    Binding {
        context: Context::Overlays,
        keys: "Esc",
        action: "close",
    },
    Binding {
        context: Context::Logs,
        keys: "e/L",
        action: "errors/close",
    },
    Binding {
        context: Context::Chooser,
        keys: "j/k/1-9/Enter/Esc",
        action: "choose/cancel",
    },
];

pub const HELP_ORDER: &[(Context, &str)] = &[
    (Context::Global, "Global"),
    (Context::Lists, "Lists"),
    (Context::SessionDetail, "Session detail"),
    (Context::Composer, "Composer"),
    (Context::RunDetail, "Run detail"),
    (Context::Attention, "Attention"),
    (Context::Overlays, "Overlays"),
];

pub fn bindings(context: Context) -> impl Iterator<Item = &'static Binding> {
    BINDINGS
        .iter()
        .filter(move |binding| binding.context == context)
}

// ---------------------------------------------------------------------
// Configurable keybindings (v1 scope: Global and Lists contexts only).
// ---------------------------------------------------------------------

/// The two remappable dispatch groups. Every other `Context` stays
/// hardcoded in v1 — see the task brief for the rationale (text input and
/// modal semantics elsewhere would risk lockout).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ContextGroup {
    Global,
    Lists,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    // Global
    FocusSessions,
    FocusRuns,
    FocusAttention,
    FocusDetail,
    NextPanel,
    PreviousPanel,
    Workspace,
    Logs,
    Help,
    Quit,
    Palette,
    // Lists
    MoveDown,
    MoveUp,
    PageDown,
    PageUp,
    Top,
    Bottom,
    Open,
    Filter,
    ToggleScope,
    Refresh,
    Yank,
    NewSession,
}

impl Action {
    pub(crate) fn group(self) -> ContextGroup {
        match self {
            Action::FocusSessions
            | Action::FocusRuns
            | Action::FocusAttention
            | Action::FocusDetail
            | Action::NextPanel
            | Action::PreviousPanel
            | Action::Workspace
            | Action::Logs
            | Action::Help
            | Action::Quit
            | Action::Palette => ContextGroup::Global,
            Action::MoveDown
            | Action::MoveUp
            | Action::PageDown
            | Action::PageUp
            | Action::Top
            | Action::Bottom
            | Action::Open
            | Action::Filter
            | Action::ToggleScope
            | Action::Refresh
            | Action::Yank
            | Action::NewSession => ContextGroup::Lists,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Action::FocusSessions => "focus_sessions",
            Action::FocusRuns => "focus_runs",
            Action::FocusAttention => "focus_attention",
            Action::FocusDetail => "focus_detail",
            Action::NextPanel => "next_panel",
            Action::PreviousPanel => "previous_panel",
            Action::Workspace => "workspace",
            Action::Logs => "logs",
            Action::Help => "help",
            Action::Quit => "quit",
            Action::Palette => "palette",
            Action::MoveDown => "move_down",
            Action::MoveUp => "move_up",
            Action::PageDown => "page_down",
            Action::PageUp => "page_up",
            Action::Top => "top",
            Action::Bottom => "bottom",
            Action::Open => "open",
            Action::Filter => "filter",
            Action::ToggleScope => "toggle_scope",
            Action::Refresh => "refresh",
            Action::Yank => "yank",
            Action::NewSession => "new_session",
        }
    }
}

/// All actions, used for name lookup and default construction.
const ALL_ACTIONS: &[Action] = &[
    Action::FocusSessions,
    Action::FocusRuns,
    Action::FocusAttention,
    Action::FocusDetail,
    Action::NextPanel,
    Action::PreviousPanel,
    Action::Workspace,
    Action::Logs,
    Action::Help,
    Action::Quit,
    Action::Palette,
    Action::MoveDown,
    Action::MoveUp,
    Action::PageDown,
    Action::PageUp,
    Action::Top,
    Action::Bottom,
    Action::Open,
    Action::Filter,
    Action::ToggleScope,
    Action::Refresh,
    Action::Yank,
    Action::NewSession,
];

/// Resolves a config-file action name (snake_case) to an `Action`.
pub fn action_by_name(name: &str) -> Option<Action> {
    ALL_ACTIONS
        .iter()
        .copied()
        .find(|action| action.name() == name)
}

fn action_name(action: Action) -> &'static str {
    action.name()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

fn combo(code: KeyCode, modifiers: KeyModifiers) -> KeyCombo {
    KeyCombo { code, modifiers }
}

/// Normalizes a live key event before matching it against a `Keymap`.
///
/// crossterm sometimes reports Shift+Tab as `Tab` with the `SHIFT`
/// modifier instead of the dedicated `BackTab` code; both are folded into
/// `BackTab` here so a single canonical combo covers either report. A
/// redundant `SHIFT` bit alongside an already-uppercase `Char` (some
/// terminals set it even though the char itself already encodes case) is
/// stripped so it does not prevent matching the canonical (unmodified)
/// combo for that letter.
fn normalize_event(key: &KeyEvent) -> KeyCombo {
    let mut code = key.code;
    let mut modifiers = key.modifiers;
    if code == KeyCode::Tab && modifiers.contains(KeyModifiers::SHIFT) {
        code = KeyCode::BackTab;
    }
    if code == KeyCode::BackTab {
        modifiers.remove(KeyModifiers::SHIFT);
    }
    if let KeyCode::Char(c) = code
        && c.is_ascii_uppercase()
        && modifiers.contains(KeyModifiers::SHIFT)
    {
        modifiers.remove(KeyModifiers::SHIFT);
    }
    KeyCombo { code, modifiers }
}

/// Parses a combo spec such as `"q"`, `"ctrl+c"`, `"shift+tab"`, `"f1"`, or
/// `"pgdn"`. Modifiers are `+`-separated and precede the key name; the key
/// name is matched case-insensitively except for bare single characters,
/// which are taken literally (so `"L"` and `"l"` are distinct combos).
pub fn parse_combo(spec: &str) -> Result<KeyCombo, String> {
    let parts: Vec<&str> = spec.split('+').filter(|part| !part.is_empty()).collect();
    if parts.is_empty() {
        return Err(format!("empty key spec `{spec}`"));
    }
    let (mods, key) = parts.split_at(parts.len() - 1);
    let key = key[0];
    let mut modifiers = KeyModifiers::NONE;
    for part in mods {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers.insert(KeyModifiers::CONTROL),
            "alt" => modifiers.insert(KeyModifiers::ALT),
            "shift" => modifiers.insert(KeyModifiers::SHIFT),
            other => return Err(format!("unknown modifier `{other}` in `{spec}`")),
        }
    }
    let lower = key.to_ascii_lowercase();
    let code = match lower.as_str() {
        "tab" => KeyCode::Tab,
        "esc" | "escape" => KeyCode::Esc,
        "enter" | "return" => KeyCode::Enter,
        "space" => KeyCode::Char(' '),
        "backspace" => KeyCode::Backspace,
        "pgup" | "pageup" => KeyCode::PageUp,
        "pgdn" | "pagedown" => KeyCode::PageDown,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        _ if key.chars().count() == 1 => {
            let ch = key.chars().next().expect("checked len");
            let mut combo = KeyCombo {
                code: KeyCode::Char(ch),
                modifiers,
            };
            if ch.is_ascii_uppercase() {
                combo.modifiers.remove(KeyModifiers::SHIFT);
            } else if ch.is_ascii_lowercase() && combo.modifiers.contains(KeyModifiers::SHIFT) {
                // `normalize_event` turns a real Shift+<letter> press into
                // `Char(<UPPERCASE>)` with `SHIFT` stripped (crossterm
                // reports it that way), so a spec like `"shift+q"` — which
                // would otherwise parse to `Char('q')` + `SHIFT` — could
                // never match. Uppercase the char and drop `SHIFT` here so
                // the parsed combo is symmetric with what a real keypress
                // normalizes to.
                combo.code = KeyCode::Char(ch.to_ascii_uppercase());
                combo.modifiers.remove(KeyModifiers::SHIFT);
            }
            return Ok(combo);
        }
        _ if lower.starts_with('f') && lower[1..].chars().all(|c| c.is_ascii_digit()) => {
            let n: u8 = lower[1..]
                .parse()
                .map_err(|_| format!("invalid function key `{key}` in `{spec}`"))?;
            KeyCode::F(n)
        }
        _ => return Err(format!("unknown key `{key}` in `{spec}`")),
    };
    if code == KeyCode::Tab && modifiers.contains(KeyModifiers::SHIFT) {
        modifiers.remove(KeyModifiers::SHIFT);
        return Ok(KeyCombo {
            code: KeyCode::BackTab,
            modifiers,
        });
    }
    Ok(KeyCombo { code, modifiers })
}

/// Runtime keybindings for the remappable `Global` and `Lists` context
/// groups. Every other context stays hardcoded (see `ContextGroup`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keymap {
    global_forward: HashMap<KeyCombo, Action>,
    global_reverse: HashMap<Action, Vec<KeyCombo>>,
    lists_forward: HashMap<KeyCombo, Action>,
    lists_reverse: HashMap<Action, Vec<KeyCombo>>,
}

impl Keymap {
    fn empty() -> Self {
        Self {
            global_forward: HashMap::new(),
            global_reverse: HashMap::new(),
            lists_forward: HashMap::new(),
            lists_reverse: HashMap::new(),
        }
    }

    fn maps_mut(
        &mut self,
        group: ContextGroup,
    ) -> (
        &mut HashMap<KeyCombo, Action>,
        &mut HashMap<Action, Vec<KeyCombo>>,
    ) {
        match group {
            ContextGroup::Global => (&mut self.global_forward, &mut self.global_reverse),
            ContextGroup::Lists => (&mut self.lists_forward, &mut self.lists_reverse),
        }
    }

    /// Resolves a live key event to an `Action` within `context_group`, or
    /// `None` if the event is unmapped there.
    pub fn action(&self, context_group: ContextGroup, key: &KeyEvent) -> Option<Action> {
        let combo = normalize_event(key);
        let forward = match context_group {
            ContextGroup::Global => &self.global_forward,
            ContextGroup::Lists => &self.lists_forward,
        };
        forward.get(&combo).copied()
    }

    /// The combos currently bound to `action`, in binding order. Used for
    /// footer/help display.
    pub fn combos(&self, action: Action) -> Vec<KeyCombo> {
        let reverse = match action.group() {
            ContextGroup::Global => &self.global_reverse,
            ContextGroup::Lists => &self.lists_reverse,
        };
        reverse.get(&action).cloned().unwrap_or_default()
    }

    /// All combos currently bound to any action in `group`. Used by
    /// config-resolution to detect cross-group and hardcoded-context
    /// collisions (see `reserved_combos`).
    pub fn bound_combos(&self, group: ContextGroup) -> HashSet<KeyCombo> {
        match group {
            ContextGroup::Global => self.global_forward.keys().copied().collect(),
            ContextGroup::Lists => self.lists_forward.keys().copied().collect(),
        }
    }

    /// Replaces `action`'s combos with `combos`, stealing any of them from
    /// whichever action currently owns them. Returns a warning for every
    /// victim action left with no combos at all. Duplicate combos in the
    /// input are collapsed.
    pub fn rebind(&mut self, action: Action, combos: Vec<KeyCombo>) -> Vec<String> {
        let group = action.group();
        let (forward, reverse) = self.maps_mut(group);
        if let Some(old) = reverse.remove(&action) {
            for combo in old {
                forward.remove(&combo);
            }
        }
        let mut warnings = Vec::new();
        let mut deduped: Vec<KeyCombo> = Vec::new();
        for combo in combos {
            if deduped.contains(&combo) {
                continue;
            }
            if let Some(&victim) = forward.get(&combo)
                && victim != action
            {
                forward.remove(&combo);
                if let Some(victim_combos) = reverse.get_mut(&victim) {
                    victim_combos.retain(|existing| existing != &combo);
                    if victim_combos.is_empty() {
                        reverse.remove(&victim);
                        warnings.push(format!("action {} left unbound", action_name(victim)));
                    }
                }
            }
            forward.insert(combo, action);
            deduped.push(combo);
        }
        if !deduped.is_empty() {
            reverse.insert(action, deduped);
        }
        warnings
    }
}

impl Default for Keymap {
    fn default() -> Self {
        let mut keymap = Keymap::empty();
        let defaults: &[(Action, &[KeyCombo])] = &[
            (
                Action::FocusSessions,
                &[combo(KeyCode::Char('1'), KeyModifiers::NONE)],
            ),
            (
                Action::FocusRuns,
                &[combo(KeyCode::Char('2'), KeyModifiers::NONE)],
            ),
            (
                Action::FocusAttention,
                &[combo(KeyCode::Char('3'), KeyModifiers::NONE)],
            ),
            (
                Action::FocusDetail,
                &[combo(KeyCode::Char('4'), KeyModifiers::NONE)],
            ),
            (
                Action::NextPanel,
                &[combo(KeyCode::Tab, KeyModifiers::NONE)],
            ),
            (
                Action::PreviousPanel,
                &[combo(KeyCode::BackTab, KeyModifiers::NONE)],
            ),
            (
                Action::Workspace,
                &[combo(KeyCode::Char('w'), KeyModifiers::NONE)],
            ),
            (
                Action::Logs,
                &[combo(KeyCode::Char('L'), KeyModifiers::NONE)],
            ),
            (
                Action::Help,
                &[
                    combo(KeyCode::Char('?'), KeyModifiers::NONE),
                    combo(KeyCode::F(1), KeyModifiers::NONE),
                ],
            ),
            (
                Action::Quit,
                &[combo(KeyCode::Char('q'), KeyModifiers::NONE)],
            ),
            (
                Action::Palette,
                &[combo(KeyCode::Char('p'), KeyModifiers::CONTROL)],
            ),
            (
                Action::MoveDown,
                &[
                    combo(KeyCode::Char('j'), KeyModifiers::NONE),
                    combo(KeyCode::Down, KeyModifiers::NONE),
                ],
            ),
            (
                Action::MoveUp,
                &[
                    combo(KeyCode::Char('k'), KeyModifiers::NONE),
                    combo(KeyCode::Up, KeyModifiers::NONE),
                ],
            ),
            (
                Action::PageDown,
                &[combo(KeyCode::PageDown, KeyModifiers::NONE)],
            ),
            (
                Action::PageUp,
                &[combo(KeyCode::PageUp, KeyModifiers::NONE)],
            ),
            (
                Action::Top,
                &[combo(KeyCode::Char('g'), KeyModifiers::NONE)],
            ),
            (
                Action::Bottom,
                &[combo(KeyCode::Char('G'), KeyModifiers::NONE)],
            ),
            (Action::Open, &[combo(KeyCode::Enter, KeyModifiers::NONE)]),
            (
                Action::Filter,
                &[combo(KeyCode::Char('/'), KeyModifiers::NONE)],
            ),
            (
                Action::ToggleScope,
                &[combo(KeyCode::Char('*'), KeyModifiers::NONE)],
            ),
            (
                Action::Refresh,
                &[combo(KeyCode::Char('r'), KeyModifiers::NONE)],
            ),
            (
                Action::Yank,
                &[combo(KeyCode::Char('y'), KeyModifiers::NONE)],
            ),
            (
                Action::NewSession,
                &[combo(KeyCode::Char('n'), KeyModifiers::NONE)],
            ),
        ];
        for (action, combos) in defaults {
            keymap.rebind(*action, combos.to_vec());
        }
        keymap
    }
}

/// Renders a combo for display, e.g. `Ctrl+C`, `Shift+Tab`, `↑`, `PgDn`,
/// `F1`, `q`.
pub fn combo_display(combo: &KeyCombo) -> String {
    let mut prefix = String::new();
    if combo.modifiers.contains(KeyModifiers::CONTROL) {
        prefix.push_str("Ctrl+");
    }
    if combo.modifiers.contains(KeyModifiers::ALT) {
        prefix.push_str("Alt+");
    }
    if combo.modifiers.contains(KeyModifiers::SHIFT) && combo.code != KeyCode::BackTab {
        prefix.push_str("Shift+");
    }
    let body = match combo.code {
        KeyCode::BackTab => "Shift+Tab".to_owned(),
        KeyCode::Tab => "Tab".to_owned(),
        KeyCode::Up => "↑".to_owned(),
        KeyCode::Down => "↓".to_owned(),
        KeyCode::Left => "←".to_owned(),
        KeyCode::Right => "→".to_owned(),
        KeyCode::PageUp => "PgUp".to_owned(),
        KeyCode::PageDown => "PgDn".to_owned(),
        KeyCode::Enter => "Enter".to_owned(),
        KeyCode::Esc => "Esc".to_owned(),
        KeyCode::Backspace => "Backspace".to_owned(),
        KeyCode::Home => "Home".to_owned(),
        KeyCode::End => "End".to_owned(),
        KeyCode::F(n) => format!("F{n}"),
        KeyCode::Char(' ') => "Space".to_owned(),
        // crossterm always reports `Ctrl+<letter>` as the lowercase char
        // (see `normalize_event`'s doc comment); uppercase it for display
        // so e.g. `ctrl+p` reads as `Ctrl+P`, matching the capitalization
        // the hardcoded contexts' `Binding.keys` text already uses for
        // their own `Ctrl+X`-style combos.
        KeyCode::Char(c)
            if combo.modifiers.contains(KeyModifiers::CONTROL) && c.is_ascii_alphabetic() =>
        {
            c.to_ascii_uppercase().to_string()
        }
        KeyCode::Char(c) => c.to_string(),
        other => format!("{other:?}"),
    };
    format!("{prefix}{body}")
}

/// Key combos hardcoded by the non-remappable contexts (Attention,
/// Session detail, Composer, Run detail, Overlays, Logs, Chooser — every
/// `Context` other than `Global`/`Lists`/`Sessions`), plus `Ctrl+C`
/// (the quit-guard shortcut, which sits ahead of the keymap lookup in
/// `key()` and is never expressed as a `Binding` row since it isn't part
/// of the `Action` system at all). Derived from `BINDINGS`' display text
/// so it can't drift from the docs table.
///
/// Used by config resolution to warn when a `[keys]` remap introduces a
/// combo that a hardcoded handler already reads for a non-remappable
/// context. Only newly *introduced* overlaps matter in practice: the
/// shipped defaults deliberately reuse a few of these combos, safely,
/// because the hardcoded handlers that own them run before the keymap
/// lookup ever sees the key (e.g. the default `logs` action is bound to
/// `L`, which the Logs overlay itself also reads to close — never a
/// conflict, since `key()` dispatches an open overlay before consulting
/// the Global keymap at all).
pub fn reserved_combos() -> HashSet<KeyCombo> {
    let mut reserved: HashSet<KeyCombo> = BINDINGS
        .iter()
        .filter(|binding| {
            !matches!(
                binding.context,
                Context::Global | Context::Lists | Context::Sessions
            )
        })
        .flat_map(|binding| parse_display_tokens(binding.keys))
        .collect();
    reserved.insert(combo(KeyCode::Char('c'), KeyModifiers::CONTROL));
    reserved
}

/// Parses a `/`-separated `Binding.keys` display string (e.g.
/// `"a/A/x/X/r/Enter/o"`, `"j/k/1-9/Enter/Esc"`) into the combos it
/// documents. Best-effort: an unrecognized token is silently skipped
/// rather than erroring, since this only feeds a warning heuristic.
fn parse_display_tokens(text: &str) -> Vec<KeyCombo> {
    let mut combos = Vec::new();
    for token in text.split('/') {
        if let Some(range) = parse_digit_range(token) {
            combos.extend(range);
            continue;
        }
        if let Some(parsed) = parse_display_token(token) {
            combos.push(parsed);
        }
    }
    combos
}

/// Parses a digit range like `"1-9"` or `"1-4"` into one combo per digit.
fn parse_digit_range(token: &str) -> Option<Vec<KeyCombo>> {
    let (start, end) = token.split_once('-')?;
    let start = start.trim().parse::<u8>().ok()?;
    let end = end.trim().parse::<u8>().ok()?;
    if start > end || end - start > 9 {
        return None;
    }
    Some(
        (start..=end)
            .map(|digit| combo(KeyCode::Char((b'0' + digit) as char), KeyModifiers::NONE))
            .collect(),
    )
}

/// Parses one `+`-joined display token, e.g. `"Ctrl+X"`, `"Alt+Enter"`,
/// `"PgUp"`, `"y"`. Unlike `parse_combo` (the user-facing config
/// grammar), this lowercases a single-letter key when `Ctrl` is present —
/// crossterm always reports `Ctrl+<letter>` as the lowercase char, so
/// `"Ctrl+X"` in a `Binding`'s display text (kept uppercase for
/// readability) must resolve to the same combo as the real key event.
fn parse_display_token(token: &str) -> Option<KeyCombo> {
    let parts: Vec<&str> = token.split('+').filter(|part| !part.is_empty()).collect();
    let (key, mods) = parts.split_last()?;
    let mut modifiers = KeyModifiers::NONE;
    for part in mods {
        match *part {
            "Ctrl" => modifiers.insert(KeyModifiers::CONTROL),
            "Alt" => modifiers.insert(KeyModifiers::ALT),
            "Shift" => modifiers.insert(KeyModifiers::SHIFT),
            _ => return None,
        }
    }
    let code = match *key {
        "Enter" => KeyCode::Enter,
        "Esc" => KeyCode::Esc,
        "Tab" => KeyCode::Tab,
        "PgUp" => KeyCode::PageUp,
        "PgDn" => KeyCode::PageDown,
        _ if key.chars().count() == 1 => {
            let mut ch = key.chars().next().expect("checked len");
            if modifiers.contains(KeyModifiers::CONTROL) {
                ch = ch.to_ascii_lowercase();
            }
            KeyCode::Char(ch)
        }
        _ => return None,
    };
    Some(KeyCombo { code, modifiers })
}

fn joined_combos(keymap: &Keymap, action: Action) -> String {
    keymap
        .combos(action)
        .iter()
        .map(combo_display)
        .collect::<Vec<_>>()
        .join("/")
}

/// The default-keymap "move" footer/help row interleaves `MoveDown`,
/// `MoveUp`, `PageUp`, `PageDown`, `Top`, and `Bottom` combos in the
/// documented order (`j/k/↑/↓/PgUp/PgDn/g/G`). Each slot names an action
/// and the index into its combo list; missing indexes (e.g. after a
/// remap that drops a combo) are silently skipped.
const MOVE_ROW_SLOTS: &[(Action, usize)] = &[
    (Action::MoveDown, 0),
    (Action::MoveUp, 0),
    (Action::MoveUp, 1),
    (Action::MoveDown, 1),
    (Action::PageUp, 0),
    (Action::PageDown, 0),
    (Action::Top, 0),
    (Action::Bottom, 0),
];

fn move_row_keys(keymap: &Keymap) -> String {
    MOVE_ROW_SLOTS
        .iter()
        .filter_map(|(action, index)| keymap.combos(*action).get(*index).map(combo_display))
        .collect::<Vec<_>>()
        .join("/")
}

fn focus_row_keys(keymap: &Keymap) -> String {
    let sessions = keymap.combos(Action::FocusSessions);
    let runs = keymap.combos(Action::FocusRuns);
    let attention = keymap.combos(Action::FocusAttention);
    let detail = keymap.combos(Action::FocusDetail);
    let is_default_digit = |combos: &[KeyCombo], digit: char| {
        combos.len() == 1
            && combos[0]
                == KeyCombo {
                    code: KeyCode::Char(digit),
                    modifiers: KeyModifiers::NONE,
                }
    };
    if is_default_digit(&sessions, '1')
        && is_default_digit(&runs, '2')
        && is_default_digit(&attention, '3')
        && is_default_digit(&detail, '4')
    {
        return "1-4".to_owned();
    }
    [
        joined_combos(keymap, Action::FocusSessions),
        joined_combos(keymap, Action::FocusRuns),
        joined_combos(keymap, Action::FocusAttention),
        joined_combos(keymap, Action::FocusDetail),
    ]
    .join("/")
}

/// A footer/help row for the remappable `Global`/`Lists` groups. `keys`
/// computes the live combo display from the current keymap; `action` is
/// the static label (labels are not remappable in v1).
struct DynamicRow {
    context: Context,
    keys: fn(&Keymap) -> String,
    action: &'static str,
}

const DYNAMIC_ROWS: &[DynamicRow] = &[
    DynamicRow {
        context: Context::Global,
        keys: focus_row_keys,
        action: "focus",
    },
    DynamicRow {
        context: Context::Global,
        keys: |keymap| {
            format!(
                "{}/{}",
                joined_combos(keymap, Action::NextPanel),
                joined_combos(keymap, Action::PreviousPanel)
            )
        },
        action: "next/previous",
    },
    DynamicRow {
        context: Context::Global,
        keys: |keymap| joined_combos(keymap, Action::Workspace),
        action: "workspace",
    },
    DynamicRow {
        context: Context::Global,
        keys: |keymap| joined_combos(keymap, Action::Logs),
        action: "logs",
    },
    DynamicRow {
        context: Context::Global,
        keys: |keymap| joined_combos(keymap, Action::Help),
        action: "help",
    },
    DynamicRow {
        context: Context::Global,
        keys: |keymap| format!("{}/Ctrl+C", joined_combos(keymap, Action::Quit)),
        action: "quit",
    },
    DynamicRow {
        context: Context::Lists,
        keys: move_row_keys,
        action: "move",
    },
    DynamicRow {
        context: Context::Lists,
        keys: |keymap| joined_combos(keymap, Action::Open),
        action: "open",
    },
    DynamicRow {
        context: Context::Lists,
        keys: |keymap| joined_combos(keymap, Action::Filter),
        action: "filter",
    },
    DynamicRow {
        context: Context::Lists,
        keys: |keymap| {
            format!(
                "{}/{}",
                joined_combos(keymap, Action::ToggleScope),
                joined_combos(keymap, Action::Refresh)
            )
        },
        action: "preset/refresh",
    },
    DynamicRow {
        context: Context::Lists,
        keys: |keymap| joined_combos(keymap, Action::Yank),
        action: "yank id",
    },
    DynamicRow {
        context: Context::Sessions,
        keys: |keymap| joined_combos(keymap, Action::NewSession),
        action: "new session",
    },
];

fn dynamic_rows(context: Context) -> impl Iterator<Item = &'static DynamicRow> {
    DYNAMIC_ROWS
        .iter()
        .filter(move |row| row.context == context)
}

/// Renders the footer strip for the given contexts. `Global`/`Lists`/
/// `Sessions` rows read their key combos from `keymap`; every other
/// context renders the static text from `BINDINGS` (not remappable in
/// v1).
pub fn footer(contexts: &[Context], keymap: &Keymap) -> String {
    let mut parts = Vec::new();
    for context in contexts {
        if matches!(
            context,
            Context::Global | Context::Lists | Context::Sessions
        ) {
            for row in dynamic_rows(*context) {
                parts.push(format!("{} {}", (row.keys)(keymap), row.action));
            }
        } else {
            for binding in bindings(*context) {
                parts.push(format!("{} {}", binding.keys, binding.action));
            }
        }
    }
    parts.join("  ")
}

pub fn help_lines(keymap: &Keymap) -> Vec<String> {
    let mut lines = Vec::new();
    for (context, title) in HELP_ORDER {
        let mut rows: Vec<(String, &'static str)> = Vec::new();
        if matches!(context, Context::Global | Context::Lists) {
            for row in dynamic_rows(*context) {
                rows.push(((row.keys)(keymap), row.action));
            }
            if *context == Context::Global {
                // `Action::Palette` intentionally isn't a footer row (Task 7
                // brief: help entry only) so it stays out of `DYNAMIC_ROWS`,
                // which `footer()` also reads from.
                rows.push((joined_combos(keymap, Action::Palette), "palette"));
            }
            if *context == Context::Lists {
                for row in dynamic_rows(Context::Sessions) {
                    rows.push(((row.keys)(keymap), row.action));
                }
            }
        } else {
            for binding in bindings(*context) {
                rows.push((binding.keys.to_owned(), binding.action));
            }
            if *context == Context::Overlays {
                for binding in bindings(Context::Logs).chain(bindings(Context::Chooser)) {
                    rows.push((binding.keys.to_owned(), binding.action));
                }
            }
        }
        if rows.is_empty() {
            continue;
        }
        lines.push((*title).to_owned());
        for (keys, action) in rows {
            lines.push(format!("  {keys:<24} {action}"));
        }
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ut_772_parse_combo_accepts_documented_forms() {
        assert_eq!(
            parse_combo("q").unwrap(),
            KeyCombo {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE
            }
        );
        assert_eq!(
            parse_combo("ctrl+c").unwrap().modifiers,
            KeyModifiers::CONTROL
        );
        assert_eq!(parse_combo("shift+tab").unwrap().code, KeyCode::BackTab);
        assert_eq!(parse_combo("f1").unwrap().code, KeyCode::F(1));
        assert_eq!(parse_combo("pgdn").unwrap().code, KeyCode::PageDown);
        assert!(parse_combo("meh").is_err());
    }

    #[test]
    fn ut_785_parse_combo_shift_letter_normalizes_to_uppercase() {
        // crossterm reports a real Shift+q press as `Char('Q')` with
        // `SHIFT` already stripped by `normalize_event` (see below), so a
        // spec of `"shift+q"` must parse to that same shape — not to
        // `Char('q')` + `SHIFT`, which `normalize_event`'s output could
        // never match.
        let combo = parse_combo("shift+q").unwrap();
        assert_eq!(
            combo,
            KeyCombo {
                code: KeyCode::Char('Q'),
                modifiers: KeyModifiers::NONE,
            }
        );
        let real_press = KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::SHIFT);
        assert_eq!(normalize_event(&real_press), combo);
    }

    #[test]
    fn ut_773_default_keymap_matches_documented_bindings() {
        let keymap = Keymap::default();
        let quit = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(
            keymap.action(ContextGroup::Global, &quit),
            Some(Action::Quit)
        );
        let filter = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
        assert_eq!(
            keymap.action(ContextGroup::Lists, &filter),
            Some(Action::Filter)
        );
    }

    #[test]
    fn ut_774_action_by_name_round_trips_every_action() {
        for action in ALL_ACTIONS {
            assert_eq!(action_by_name(action.name()), Some(*action));
        }
        assert_eq!(action_by_name("nope"), None);
    }

    #[test]
    fn ut_775_default_footer_matches_documented_string() {
        let keymap = Keymap::default();
        assert_eq!(
            footer(&[Context::Global], &keymap),
            "1-4 focus  Tab/Shift+Tab next/previous  w workspace  L logs  ?/F1 help  q/Ctrl+C quit"
        );
        assert_eq!(
            footer(
                &[Context::Global, Context::Lists, Context::Sessions],
                &keymap
            ),
            "1-4 focus  Tab/Shift+Tab next/previous  w workspace  L logs  ?/F1 help  q/Ctrl+C quit  j/k/↑/↓/PgUp/PgDn/g/G move  Enter open  / filter  */r preset/refresh  y yank id  n new session"
        );
    }

    #[test]
    fn ut_776_rebind_steals_combo_and_warns_when_victim_unbound() {
        let mut keymap = Keymap::default();
        let warnings = keymap.rebind(
            Action::Refresh,
            vec![combo(KeyCode::Char('y'), KeyModifiers::NONE)],
        );
        assert_eq!(warnings, vec!["action yank left unbound".to_owned()]);
        let yank = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
        assert_eq!(
            keymap.action(ContextGroup::Lists, &yank),
            Some(Action::Refresh),
            "yank's only combo was stolen by refresh"
        );
        assert!(keymap.combos(Action::Yank).is_empty());
    }

    #[test]
    fn ut_777_normalize_event_folds_shift_tab_variants() {
        let via_tab = normalize_event(&KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT));
        let via_backtab = normalize_event(&KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE));
        assert_eq!(via_tab, via_backtab);
    }

    #[test]
    fn ut_779_reserved_combos_cover_hardcoded_contexts_and_ctrl_c() {
        let reserved = reserved_combos();
        // Attention's `a` (approve) and Ctrl+C (quit-guard) are both
        // hardcoded outside the Global/Lists keymap.
        assert!(reserved.contains(&combo(KeyCode::Char('a'), KeyModifiers::NONE)));
        assert!(reserved.contains(&combo(KeyCode::Char('c'), KeyModifiers::CONTROL)));
        // "Ctrl+X" in SessionDetail's display text must resolve to the
        // real (lowercase) physical combo, not the display capitalization.
        assert!(reserved.contains(&combo(KeyCode::Char('x'), KeyModifiers::CONTROL)));
        // Chooser's "1-9" range expands to individual digit combos.
        assert!(reserved.contains(&combo(KeyCode::Char('5'), KeyModifiers::NONE)));
        // Global/Lists/Sessions rows never contribute (those are the
        // remappable groups, not hardcoded reserves).
        assert!(!reserved.contains(&combo(KeyCode::Char('w'), KeyModifiers::NONE)));
    }

    #[test]
    fn ut_780_default_keymap_never_collides_across_groups_or_with_reserved_beyond_documented_overlap()
     {
        let keymap = Keymap::default();
        let global = keymap.bound_combos(ContextGroup::Global);
        let lists = keymap.bound_combos(ContextGroup::Lists);
        assert!(
            global.is_disjoint(&lists),
            "Global and Lists defaults must never collide (Global is checked first)"
        );
    }
}
