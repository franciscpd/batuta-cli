use crate::{
    app::{
        model::{Model, Panel},
        panels::sessions,
    },
    cmd::{Cmd, Request},
};
use compozy_client::types::{PromptMode, PromptRequest, PromptRuntime};

pub(super) fn toggle_agent(model: &mut Model) -> Vec<Cmd> {
    model.sessions_all_agents = !model.sessions_all_agents;
    model.dirty = true;
    sessions::request(model).into_iter().collect()
}

pub(super) fn refresh(model: &mut Model) -> Vec<Cmd> {
    sessions::request(model).into_iter().collect()
}

pub(super) fn create(model: &mut Model) -> Vec<Cmd> {
    if model.create_session_pending {
        return Vec::new();
    }
    let Some(workspace) = model.workspace.as_ref().map(|value| value.id.clone()) else {
        return Vec::new();
    };
    let agent = model.settings.preset.agent.clone();
    model.create_session_pending = true;
    let request = model.allocate(|id| Request::CreateSession {
        id,
        workspace,
        agent,
    });
    vec![Cmd::Post(request)]
}

pub(super) fn send_prompt(model: &mut Model) -> Vec<Cmd> {
    if model.prompt_pending {
        return Vec::new();
    }
    let Some(workspace) = model.workspace.as_ref().map(|value| value.id.clone()) else {
        return Vec::new();
    };
    let Some((session, message)) = model
        .session_detail()
        .map(|detail| (detail.session.id.clone(), detail.composer.text.clone()))
    else {
        return Vec::new();
    };
    if message.is_empty() {
        return Vec::new();
    }
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
    let (message_id, idempotency_key) = model.message_ids();
    let prompt = PromptRequest {
        message,
        message_id,
        idempotency_key,
        mode: PromptMode::Queue,
        expected_turn_id: None,
        runtime,
    };
    model.prompt_pending = true;
    model.focus = Panel::Detail;
    let request = model.allocate(|id| Request::Prompt {
        id,
        workspace,
        session,
        prompt,
    });
    vec![Cmd::Post(request)]
}
