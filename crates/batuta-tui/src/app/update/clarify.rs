use crate::{
    app::model::{Model, Overlay},
    cmd::{Cmd, Request},
};
use compozy_client::types::ClarifyAnswer;
use crossterm::event::{KeyCode, KeyEvent};

pub(super) fn key(model: &mut Model, key: KeyEvent) -> Vec<Cmd> {
    let Some(Overlay::Clarify {
        session_id,
        request_id,
        text,
        choices,
        selected,
        hint,
        ..
    }) = &mut model.overlay
    else {
        return Vec::new();
    };
    match key.code {
        KeyCode::Esc => {
            model.overlay = None;
            model.dirty = true;
            Vec::new()
        }
        KeyCode::Char(c @ '1'..='9') if !choices.is_empty() => {
            let index = usize::from(c as u8 - b'1');
            if index < choices.len() {
                *selected = index;
            }
            model.dirty = true;
            Vec::new()
        }
        KeyCode::Char('j') | KeyCode::Down if !choices.is_empty() => {
            *selected = (*selected + 1).min(choices.len() - 1);
            model.dirty = true;
            Vec::new()
        }
        KeyCode::Char('k') | KeyCode::Up if !choices.is_empty() => {
            *selected = selected.saturating_sub(1);
            model.dirty = true;
            Vec::new()
        }
        KeyCode::Backspace if choices.is_empty() => {
            text.pop();
            model.dirty = true;
            Vec::new()
        }
        KeyCode::Char(c) if choices.is_empty() => {
            text.push(c);
            *hint = None;
            model.dirty = true;
            Vec::new()
        }
        KeyCode::Enter => {
            let answer = if choices.is_empty() {
                if text.trim().is_empty() {
                    *hint = Some("type an answer".into());
                    model.dirty = true;
                    return Vec::new();
                }
                ClarifyAnswer::Text(text.clone())
            } else {
                ClarifyAnswer::Choice(*selected)
            };
            let session = session_id.clone();
            let request_id = request_id.clone();
            if model.refuse_write("answer") {
                return Vec::new();
            }
            let Some(workspace) = model.workspace.as_ref().map(|value| value.id.clone()) else {
                return Vec::new();
            };
            let request = model.allocate(|id| Request::AnswerClarification {
                id,
                workspace,
                session,
                request_id,
                answer,
            });
            vec![Cmd::Post(request)]
        }
        _ => Vec::new(),
    }
}
