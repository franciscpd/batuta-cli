use crate::{
    app::{model::Model, panels::sessions},
    cmd::{Cmd, Request},
};

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
