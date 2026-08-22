use crate::{
    app::model::{Model, Overlay, Panel},
    cmd::Cmd,
    keymap::{Action, ContextGroup},
};
use crossterm::event::{KeyCode, KeyEvent};

/// One row in the command palette (`Ctrl+P`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaletteEntry {
    pub label: &'static str,
    pub action: Action,
}

/// Static catalog of every action reachable from the palette, in display
/// order. Filtered case-insensitively by substring on `label`.
const CATALOG: &[PaletteEntry] = &[
    PaletteEntry {
        label: "focus: sessions",
        action: Action::FocusSessions,
    },
    PaletteEntry {
        label: "focus: deliver runs",
        action: Action::FocusRuns,
    },
    PaletteEntry {
        label: "focus: attention",
        action: Action::FocusAttention,
    },
    PaletteEntry {
        label: "focus: detail",
        action: Action::FocusDetail,
    },
    PaletteEntry {
        label: "workspace: switch",
        action: Action::Workspace,
    },
    PaletteEntry {
        label: "logs: open overlay",
        action: Action::Logs,
    },
    PaletteEntry {
        label: "list: toggle all agents/loops",
        action: Action::ToggleScope,
    },
    PaletteEntry {
        label: "list: refresh now",
        action: Action::Refresh,
    },
    PaletteEntry {
        label: "session: new",
        action: Action::NewSession,
    },
    PaletteEntry {
        label: "help",
        action: Action::Help,
    },
    PaletteEntry {
        label: "quit",
        action: Action::Quit,
    },
];

/// Entries matching `query` (case-insensitive substring on `label`). The
/// palette adds no new state rules — every entry dispatches exactly as its
/// key equivalent would, so `entries` doesn't need to inspect `model` today;
/// it takes `&Model` to leave room for state-aware filtering later without
/// changing the call site.
pub fn entries(_model: &Model, query: &str) -> Vec<PaletteEntry> {
    let query = query.to_ascii_lowercase();
    CATALOG
        .iter()
        .copied()
        .filter(|entry| entry.label.to_ascii_lowercase().contains(&query))
        .collect()
}

/// Opens the palette with an empty query and no selection.
pub(super) fn open(model: &mut Model) -> Vec<Cmd> {
    model.overlay = Some(Overlay::Palette {
        query: String::new(),
        selected: 0,
    });
    model.dirty = true;
    Vec::new()
}

/// Routes a key event while the palette overlay is open. Typing filters the
/// query; `j`/`k`/arrows move the selection; `Enter` closes the overlay
/// first, then dispatches the selected entry's action exactly as its key
/// equivalent would; `Esc` closes without dispatching.
pub(super) fn key(model: &mut Model, key: KeyEvent) -> Vec<Cmd> {
    let Some(Overlay::Palette { query, selected }) = &model.overlay else {
        return Vec::new();
    };
    let mut query = query.clone();
    let mut selected = *selected;
    match key.code {
        KeyCode::Esc => {
            model.overlay = None;
            model.dirty = true;
            return Vec::new();
        }
        KeyCode::Enter => {
            let action = entries(model, &query)
                .get(selected)
                .map(|entry| entry.action);
            model.overlay = None;
            model.dirty = true;
            return dispatch(model, action);
        }
        KeyCode::Char('j') | KeyCode::Down => {
            let len = entries(model, &query).len();
            if len > 0 {
                selected = (selected + 1).min(len - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            selected = selected.saturating_sub(1);
        }
        KeyCode::Backspace => {
            query.pop();
            selected = 0;
        }
        KeyCode::Char(c) => {
            query.push(c);
            selected = 0;
        }
        _ => return Vec::new(),
    }
    model.overlay = Some(Overlay::Palette { query, selected });
    model.dirty = true;
    Vec::new()
}

/// Dispatches `action` exactly as its key equivalent would: global actions
/// via `apply_global_action`, list actions via `apply_list_action` (which
/// act on the currently focused list — `NewSession` focuses Sessions
/// first).
fn dispatch(model: &mut Model, action: Option<Action>) -> Vec<Cmd> {
    let Some(action) = action else {
        return Vec::new();
    };
    match action.group() {
        ContextGroup::Global => super::keys::apply_global_action(model, action),
        ContextGroup::Lists => {
            if action == Action::NewSession {
                model.focus = Panel::Sessions;
            }
            super::keys::apply_list_action(model, action)
        }
    }
}
