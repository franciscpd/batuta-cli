use crate::{
    app::{
        model::{AttentionSource, Model, Overlay},
        panels::attention,
    },
    cmd::{Cmd, Request},
};
use compozy_client::{
    TaskVerb,
    types::{ApproveRequest, Decision},
};
use crossterm::event::{KeyCode, KeyEvent};

pub(super) fn key(model: &mut Model, key: KeyEvent) -> Vec<Cmd> {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if !model.attention.is_empty() {
                model.attention_selected = Some(
                    (model.attention_selected.unwrap_or(0) + 1).min(model.attention.len() - 1),
                );
                model.dirty = true;
            }
            Vec::new()
        }
        KeyCode::Char('k') | KeyCode::Up => {
            model.attention_selected =
                Some(model.attention_selected.unwrap_or(0).saturating_sub(1));
            model.dirty = true;
            Vec::new()
        }
        KeyCode::Char('a') => verb(model, Decision::AllowOnce),
        KeyCode::Char('x') => verb(model, Decision::RejectOnce),
        KeyCode::Char('A') => persistent(model, Decision::AllowAlways),
        KeyCode::Char('X') => persistent(model, Decision::RejectAlways),
        KeyCode::Char('r') => task_verb(model, TaskVerb::Retry),
        KeyCode::Enter | KeyCode::Char('o') => open(model),
        _ => Vec::new(),
    }
}

fn persistent(model: &mut Model, decision: Decision) -> Vec<Cmd> {
    let Some(index) = model.attention_selected else {
        return Vec::new();
    };
    let allowed = matches!(
        attention::selected(model).and_then(|item| item.source.as_ref()),
        Some(AttentionSource::Permission {
            allow_always: true,
            ..
        })
    );
    if allowed {
        model.attention_confirm = Some((index, decision));
        model.dirty = true;
    }
    Vec::new()
}

pub(super) fn confirm(model: &mut Model) -> Vec<Cmd> {
    let Some((index, decision)) = model.attention_confirm.take() else {
        return Vec::new();
    };
    model.attention_selected = Some(index);
    verb(model, decision)
}

pub(super) fn inline_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    let decision = match key.code {
        KeyCode::Char('a') => Decision::AllowOnce,
        KeyCode::Char('x') => Decision::RejectOnce,
        KeyCode::Char('A') => Decision::AllowAlways,
        KeyCode::Char('X') => Decision::RejectAlways,
        _ => return None,
    };
    let (session, permission) = model.session_detail().and_then(|detail| {
        let entries = detail.transcript.entries();
        let entry = entries.get(detail.view.selection)?;
        let permission = entry.message.parts.iter().find_map(|part| {
            compozy_client::types::PermissionData::from_part(part).and_then(Result::ok)
        })?;
        Some((detail.session.id.clone(), permission))
    })?;
    let allow_always = permission
        .raw
        .options
        .iter()
        .any(|option| option.decision == decision_wire(decision));
    if matches!(decision, Decision::AllowAlways | Decision::RejectAlways) && !allow_always {
        return Some(Vec::new());
    }
    let request = ApproveRequest {
        request_id: permission.request_id,
        turn_id: permission.turn_id,
        decision,
    };
    if matches!(decision, Decision::AllowAlways | Decision::RejectAlways) {
        model.inline_permission_confirm = Some((session, request));
        model.dirty = true;
        Some(Vec::new())
    } else {
        Some(post_approve(model, session, request))
    }
}

pub(super) fn confirm_inline(model: &mut Model) -> Vec<Cmd> {
    let Some((session, request)) = model.inline_permission_confirm.take() else {
        return Vec::new();
    };
    post_approve(model, session, request)
}

fn post_approve(model: &mut Model, session: String, request: ApproveRequest) -> Vec<Cmd> {
    let verb = match request.decision {
        Decision::AllowOnce | Decision::AllowAlways => "approve",
        Decision::RejectOnce | Decision::RejectAlways => "reject",
    };
    if model.refuse_write(verb) {
        return Vec::new();
    }
    let Some(workspace) = model.workspace.as_ref().map(|value| value.id.clone()) else {
        return Vec::new();
    };
    let request = model.allocate(|id| Request::Approve {
        id,
        workspace,
        session,
        request,
    });
    vec![Cmd::Post(request)]
}

fn decision_wire(decision: Decision) -> &'static str {
    match decision {
        Decision::AllowOnce => "allow-once",
        Decision::AllowAlways => "allow-always",
        Decision::RejectOnce => "reject-once",
        Decision::RejectAlways => "reject-always",
    }
}

fn verb(model: &mut Model, decision: Decision) -> Vec<Cmd> {
    let Some(item) = attention::selected(model).cloned() else {
        return Vec::new();
    };
    match item.source {
        Some(AttentionSource::Permission {
            session_id,
            request_id,
            turn_id,
            allow_always,
        }) => {
            if matches!(decision, Decision::AllowAlways | Decision::RejectAlways) && !allow_always {
                return Vec::new();
            }
            let verb = match decision {
                Decision::AllowOnce | Decision::AllowAlways => "approve",
                Decision::RejectOnce | Decision::RejectAlways => "reject",
            };
            if model.refuse_write(verb) {
                return Vec::new();
            }
            let Some(workspace) = model.workspace.as_ref().map(|value| value.id.clone()) else {
                return Vec::new();
            };
            let request = model.allocate(|id| Request::Approve {
                id,
                workspace,
                session: session_id,
                request: ApproveRequest {
                    request_id,
                    turn_id,
                    decision,
                },
            });
            vec![Cmd::Post(request)]
        }
        Some(AttentionSource::Overview { actions, .. }) => {
            let task = match decision {
                Decision::AllowOnce if actions.iter().any(|action| action == "approve") => {
                    TaskVerb::Approve
                }
                Decision::RejectOnce if actions.iter().any(|action| action == "reject") => {
                    TaskVerb::Reject
                }
                _ => return Vec::new(),
            };
            task_verb(model, task)
        }
        _ => Vec::new(),
    }
}

fn task_verb(model: &mut Model, verb: TaskVerb) -> Vec<Cmd> {
    let Some(AttentionSource::Overview {
        task_id,
        actions,
        run_id,
        ..
    }) = attention::selected(model).and_then(|item| item.source.clone())
    else {
        return Vec::new();
    };
    let wire = match verb {
        TaskVerb::Approve => "approve",
        TaskVerb::Reject => "reject",
        TaskVerb::Retry => "retry",
    };
    if !actions.iter().any(|action| action == wire) || (verb == TaskVerb::Retry && run_id.is_none())
    {
        return Vec::new();
    }
    if model.refuse_write(wire) {
        return Vec::new();
    }
    let Some(workspace) = model.workspace.as_ref().map(|value| value.id.clone()) else {
        return Vec::new();
    };
    let request = model.allocate(|id| Request::TaskVerb {
        id,
        workspace,
        task_id,
        verb,
    });
    vec![Cmd::Post(request)]
}

fn open(model: &mut Model) -> Vec<Cmd> {
    let Some(item) = attention::selected(model).cloned() else {
        return Vec::new();
    };
    match item.source {
        Some(AttentionSource::Clarification {
            session_id,
            request_id,
            choices,
            deadline,
        }) => {
            let commands = super::keys::open_session_id(model, session_id.clone());
            model.overlay = Some(Overlay::Clarify {
                session_id,
                request_id,
                question: item.title,
                deadline,
                text: String::new(),
                choices: choices.clone(),
                selected: 0,
                text_focused: choices.is_empty(),
                hint: None,
            });
            model.dirty = true;
            commands
        }
        Some(AttentionSource::Overview {
            actions,
            task_id,
            session_id,
            run_id,
            ..
        }) => {
            if !actions.iter().any(|action| action == "open") {
                return Vec::new();
            }
            if let Some(session) = session_id {
                super::keys::open_session_id(model, session)
            } else if let Some(run) = run_id {
                super::keys::open_run_id(model, run)
            } else {
                model.set_sticky_toast(format!(
                    "task {task_id} — open in web: http://localhost:2123/…"
                ));
                Vec::new()
            }
        }
        Some(AttentionSource::Permission { session_id, .. }) => {
            super::keys::open_session_id(model, session_id)
        }
        None => {
            if let Some(session) = item.session_id {
                super::keys::open_session_id(model, session)
            } else if let Some(run) = item.run_id {
                super::keys::open_run_id(model, run)
            } else {
                Vec::new()
            }
        }
    }
}

pub(super) fn refresh(model: &mut Model) -> Vec<Cmd> {
    attention::refresh(model)
}

pub(super) fn cancel_confirm(model: &mut Model) {
    model.attention_confirm = None;
    model.inline_permission_confirm = None;
    model.dirty = true;
}
