use crate::{
    app::{ModeChooser, Model, Panel},
    cmd::{Cmd, Request},
};
use compozy_client::types::{PromptMode, PromptRequest, PromptRuntime};
use crossterm::event::{KeyCode, KeyEvent};

const MAX_PROMPT_BYTES: usize = 64 * 1024;

pub(super) fn send(model: &mut Model) -> Vec<Cmd> {
    if model.prompt_pending {
        return Vec::new();
    }
    let Some(detail) = model.session_detail() else {
        return Vec::new();
    };
    if detail.composer.is_empty() {
        return Vec::new();
    }
    if detail.composer.text().len() > MAX_PROMPT_BYTES {
        model.set_sticky_toast("prompt too long (max 64 KiB)");
        return Vec::new();
    }
    if busy(model) {
        model.session_detail_mut().unwrap().chooser = Some(ModeChooser { selected: 0 });
        model.dirty = true;
        return Vec::new();
    }
    post(model, PromptMode::Queue)
}

pub(super) fn chooser_key(model: &mut Model, key: KeyEvent) -> Vec<Cmd> {
    match key.code {
        KeyCode::Esc => {
            model.session_detail_mut().unwrap().chooser = None;
            model.dirty = true;
            Vec::new()
        }
        KeyCode::Char('j') | KeyCode::Down => {
            let chooser = model
                .session_detail_mut()
                .unwrap()
                .chooser
                .as_mut()
                .unwrap();
            chooser.selected = (chooser.selected + 1).min(2);
            model.dirty = true;
            Vec::new()
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let chooser = model
                .session_detail_mut()
                .unwrap()
                .chooser
                .as_mut()
                .unwrap();
            chooser.selected = chooser.selected.saturating_sub(1);
            model.dirty = true;
            Vec::new()
        }
        KeyCode::Char('1' | '2' | '3') => {
            let selected = match key.code {
                KeyCode::Char('1') => 0,
                KeyCode::Char('2') => 1,
                _ => 2,
            };
            model
                .session_detail_mut()
                .unwrap()
                .chooser
                .as_mut()
                .unwrap()
                .selected = selected;
            model.dirty = true;
            Vec::new()
        }
        KeyCode::Enter => {
            let selected = model
                .session_detail()
                .unwrap()
                .chooser
                .as_ref()
                .unwrap()
                .selected;
            let mode = [PromptMode::Queue, PromptMode::Steer, PromptMode::Interrupt][selected];
            model.session_detail_mut().unwrap().chooser = None;
            post(model, mode)
        }
        _ => Vec::new(),
    }
}

fn post(model: &mut Model, mode: PromptMode) -> Vec<Cmd> {
    let Some(workspace) = model.workspace.as_ref().map(|value| value.id.clone()) else {
        return Vec::new();
    };
    let (session, message, expected_turn_id) = {
        let detail = model.session_detail().unwrap();
        (
            detail.session.id.clone(),
            detail.composer.text(),
            busy(model)
                .then(|| {
                    detail
                        .session
                        .activity
                        .as_ref()
                        .and_then(|activity| activity.turn_id.clone())
                })
                .flatten(),
        )
    };
    let runtime = if model.app_created_sessions.contains(&session) {
        if model.settings.preset.provider.is_empty() {
            model.set_sticky_toast("set preset.provider in config");
            return Vec::new();
        }
        Some(PromptRuntime {
            provider: model.settings.preset.provider.clone(),
            model: (!model.settings.preset.model.is_empty())
                .then(|| model.settings.preset.model.clone()),
        })
    } else {
        None
    };
    let reused = model
        .failed_prompt
        .as_ref()
        .filter(|(failed_session, prompt)| failed_session == &session && prompt.message == message)
        .map(|(_, prompt)| (prompt.message_id.clone(), prompt.idempotency_key.clone()));
    let (message_id, idempotency_key) = reused.unwrap_or_else(|| model.message_ids());
    let prompt = PromptRequest {
        message,
        message_id,
        idempotency_key,
        mode,
        expected_turn_id,
        runtime,
    };
    model.failed_prompt = None;
    model.prompt_pending = true;
    model.focus = Panel::Detail;
    let request = model.allocate(|id| Request::Prompt {
        id,
        workspace,
        session,
        prompt,
        consume_body: false,
    });
    vec![Cmd::Post(request)]
}

pub(super) fn busy(model: &Model) -> bool {
    model.session_detail().is_some_and(|detail| {
        detail.session.badge.as_deref() == Some("running")
            || detail
                .session
                .activity
                .as_ref()
                .and_then(|activity| activity.turn_id.as_ref())
                .is_some()
    })
}
