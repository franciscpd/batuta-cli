use crate::{app::Model, cmd::Cmd};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(super) fn key(model: &mut Model, key: KeyEvent) -> Vec<Cmd> {
    let Some(detail) = model.session_detail_mut() else {
        return Vec::new();
    };
    match key.code {
        KeyCode::Esc => detail.composer.focused = false,
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            detail.composer.clear();
        }
        KeyCode::Enter
            if key
                .modifiers
                .intersects(KeyModifiers::ALT | KeyModifiers::SHIFT) =>
        {
            detail
                .composer
                .insert(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        }
        KeyCode::Enter => return super::prompt::send(model),
        _ => {
            detail.composer.insert(key);
        }
    }
    model.dirty = true;
    Vec::new()
}
