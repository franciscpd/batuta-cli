use crate::{
    app::model::{AttentionItem, AttentionSource, Detail, Model},
    cmd::{Cmd, Request},
};
use compozy_client::types::{Entry, Part, PermissionData, TranscriptPage};

fn permissions(entries: &[Entry]) -> Vec<PermissionData> {
    entries
        .iter()
        .flat_map(|entry| &entry.message.parts)
        .filter_map(|part| PermissionData::from_part(part).and_then(Result::ok))
        .filter(|permission| permission.decision.is_none())
        .collect()
}

pub fn apply_transcript(model: &mut Model, session: &str, page: &TranscriptPage) -> Vec<Cmd> {
    model
        .attention_permissions
        .insert(session.to_owned(), permissions(&page.entries));
    rebuild(model)
}

pub fn sync_open_detail(model: &mut Model) {
    let Some((session, permissions)) = model.session_detail().map(|detail| {
        (
            detail.session.id.clone(),
            detail
                .transcript
                .entries()
                .into_iter()
                .flat_map(|entry| &entry.message.parts)
                .filter_map(|part| PermissionData::from_part(part).and_then(Result::ok))
                .filter(|permission| permission.decision.is_none())
                .collect::<Vec<_>>(),
        )
    }) else {
        return;
    };
    model.attention_permissions.insert(session, permissions);
}

pub fn refresh(model: &mut Model) -> Vec<Cmd> {
    let Some(workspace) = model.workspace.as_ref().map(|value| value.id.clone()) else {
        return Vec::new();
    };
    sync_open_detail(model);
    let open = model
        .session_detail()
        .map(|detail| detail.session.id.clone());
    let session_ids = model
        .sessions_unfiltered
        .iter()
        .map(|row| row.id.clone())
        .collect::<Vec<_>>();
    let mut commands = Vec::with_capacity(session_ids.len() * 2 + 1);
    let overview = model.allocate(|id| Request::Overview {
        id,
        workspace: workspace.clone(),
    });
    commands.push(Cmd::Get(overview));
    for session in session_ids {
        let clarification = model.allocate(|id| Request::Clarifications {
            id,
            workspace: workspace.clone(),
            session: session.clone(),
        });
        commands.push(Cmd::Get(clarification));
        if open.as_deref() != Some(session.as_str()) {
            let transcript = model.allocate(|id| Request::VisibleTranscript {
                id,
                workspace: workspace.clone(),
                session,
            });
            commands.push(Cmd::Get(transcript));
        }
    }
    commands
}

/// Pushes `Cmd::Notify` when the attention list grew, the terminal is
/// unfocused, and notifications are enabled; otherwise a no-op.
fn notify_on_growth(model: &Model, before: usize, commands: &mut Vec<Cmd>) {
    let after = model.attention.len();
    if after > before && !model.terminal_focused && model.settings.ui.notify {
        commands.push(Cmd::Notify {
            body: format!(
                "batuta: {after} attention item{}",
                if after == 1 { "" } else { "s" }
            ),
        });
    }
}

pub fn rebuild(model: &mut Model) -> Vec<Cmd> {
    let before = model.attention.len();
    let visible = model
        .sessions_unfiltered
        .iter()
        .map(|row| row.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut items = Vec::new();
    for (session, permissions) in &model.attention_permissions {
        if !visible.contains(session.as_str()) {
            continue;
        }
        for permission in permissions {
            let allow_always =
                permission.raw.options.iter().any(|option| {
                    matches!(option.decision.as_str(), "allow-always" | "reject-always")
                });
            items.push(AttentionItem {
                source: Some(AttentionSource::Permission {
                    session_id: session.clone(),
                    request_id: permission.request_id.clone(),
                    turn_id: permission.turn_id.clone(),
                    allow_always,
                }),
                title: permission
                    .title
                    .clone()
                    .or_else(|| permission.action.clone())
                    .unwrap_or_else(|| "permission".into()),
                detail: input_summary(&permission.raw.tool_input),
                session_id: Some(session.clone()),
                run_id: None,
                at: None,
            });
        }
    }
    for (session, clarifications) in &model.attention_clarifications {
        if !visible.contains(session.as_str()) {
            continue;
        }
        for clarification in clarifications {
            items.push(AttentionItem {
                source: Some(AttentionSource::Clarification {
                    session_id: session.clone(),
                    request_id: clarification.request_id.clone(),
                    choices: clarification.choices.clone(),
                    deadline: clarification.deadline.clone(),
                }),
                title: clarification.question.clone(),
                detail: clarification.agent_name.clone(),
                session_id: Some(session.clone()),
                run_id: None,
                at: clarification.asked_at.clone(),
            });
        }
    }
    for item in &model.attention_overview {
        items.push(AttentionItem {
            source: Some(AttentionSource::Overview {
                kind: item.kind.clone(),
                task_id: item.task_id.clone(),
                run_id: (!item.run_id.is_empty()).then(|| item.run_id.clone()),
                session_id: (!item.session_id.is_empty()).then(|| item.session_id.clone()),
                actions: item.actions.clone(),
            }),
            title: item.title.clone(),
            detail: item.detail.clone(),
            session_id: (!item.session_id.is_empty()).then(|| item.session_id.clone()),
            run_id: (!item.run_id.is_empty()).then(|| item.run_id.clone()),
            at: Some(item.occurred_at.clone()),
        });
    }
    items.sort_by(|left, right| {
        left.at
            .as_ref()
            .map(|value| value.raw())
            .unwrap_or("")
            .cmp(right.at.as_ref().map(|value| value.raw()).unwrap_or(""))
    });
    model.attention = items;
    model.attention_selected = if model.attention.is_empty() {
        None
    } else {
        Some(
            model
                .attention_selected
                .unwrap_or(0)
                .min(model.attention.len() - 1),
        )
    };
    let mut commands = Vec::new();
    notify_on_growth(model, before, &mut commands);
    commands
}

pub fn permission_from_delta(parts: &[Part]) -> bool {
    parts
        .iter()
        .any(|part| matches!(part, Part::Permission { .. }))
}

fn input_summary(input: &serde_json::Value) -> String {
    match input {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Object(values) => values
            .values()
            .find_map(serde_json::Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| input.to_string()),
        _ => input.to_string(),
    }
}

pub fn selected(model: &Model) -> Option<&AttentionItem> {
    model
        .attention_selected
        .and_then(|index| model.attention.get(index))
}

pub fn source_glyph(item: &AttentionItem) -> &'static str {
    match item.source {
        Some(AttentionSource::Permission { .. }) => "▪",
        Some(AttentionSource::Clarification { .. }) => "?",
        Some(AttentionSource::Overview { .. }) => "!",
        None => "!",
    }
}

pub fn confirm_prompt(model: &Model) -> Option<&'static str> {
    model.attention_confirm.map(|(_, decision)| match decision {
        compozy_client::types::Decision::AllowAlways => "allow always for this tool? Enter/Esc",
        compozy_client::types::Decision::RejectAlways => "reject always for this tool? Enter/Esc",
        _ => "confirm? Enter/Esc",
    })
}

pub fn allowed_verbs(model: &Model) -> Option<String> {
    let item = selected(model)?;
    Some(match item.source.as_ref()? {
        AttentionSource::Permission { allow_always, .. } => {
            if *allow_always {
                "a allow · x reject · A/X always".into()
            } else {
                "a allow · x reject".into()
            }
        }
        AttentionSource::Clarification { .. } => "Enter answer".into(),
        AttentionSource::Overview { actions, .. } => actions.join(" · "),
    })
}

pub fn detail_matches_session(model: &Model, session: &str) -> bool {
    matches!(&model.detail, Detail::Session(detail) if detail.session.id == session)
}
