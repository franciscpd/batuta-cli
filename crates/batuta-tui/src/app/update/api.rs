use crate::{
    app::{
        model::{
            Detail, Model, Panel, PendingKind, SessionDetail, StreamStatus, WorkspaceRef,
            page_into_detail,
        },
        panels::{attention, runs, sessions},
    },
    cmd::{Cmd, Request, RequestId, StreamId, TimerId},
    msg::{ApiResponse, ApiResult},
};
use compozy_client::{
    TaskVerb,
    types::{Decision, PromptOutcome},
};
use std::time::Duration;

pub(super) fn api(model: &mut Model, id: RequestId, result: ApiResult) -> Vec<Cmd> {
    let Some(PendingKind::Request(request)) = model.pending.remove(&id) else {
        return Vec::new();
    };
    match result {
        Err(error) => failure(model, request, error),
        Ok(response) => {
            model.daemon.poll_ok = true;
            apply(model, request, response)
        }
    }
}

fn failure(model: &mut Model, request: Request, error: String) -> Vec<Cmd> {
    model.dirty = true;
    match &request {
        Request::Status { .. } => {
            model.daemon.poll_ok = false;
            Vec::new()
        }
        Request::Runs { .. } => {
            model.runs_stale = true;
            model.set_sticky_toast(error);
            runs::any_live(model)
                .then(|| Cmd::After(Duration::from_secs(5), TimerId::RunsPoll))
                .into_iter()
                .collect()
        }
        Request::Overview { .. } if unavailable(&error) => {
            model.attention_overview_unavailable = true;
            attention::rebuild(model);
            Vec::new()
        }
        Request::CreateSession { agent, .. } => {
            model.create_session_pending = false;
            let text = if unavailable(&error) {
                "daemon is draining".into()
            } else if error.contains("400") || error.to_lowercase().contains("agent") {
                format!("agent {agent} not found in this workspace")
            } else {
                error
            };
            model.set_sticky_toast(text);
            Vec::new()
        }
        Request::Prompt {
            session, prompt, ..
        } => {
            model.prompt_pending = false;
            model.failed_prompt = Some((session.clone(), prompt.clone()));
            model.set_sticky_toast(prompt_failure(&error));
            Vec::new()
        }
        Request::CancelPrompt { .. } => {
            model.set_sticky_toast(clean_error(&error));
            Vec::new()
        }
        Request::Approve {
            session, request, ..
        } => {
            if not_found(&error) {
                if let Some(items) = model.attention_permissions.get_mut(session) {
                    items.retain(|item| item.request_id != request.request_id);
                }
                attention::rebuild(model);
            }
            model.set_sticky_toast(if conflict(&error) {
                "already decided".into()
            } else {
                error
            });
            attention::refresh(model)
        }
        Request::AnswerClarification { .. } => {
            model.overlay = None;
            model.set_sticky_toast(if not_found(&error) {
                "clarification expired or already answered".into()
            } else if unavailable(&error) {
                "clarification service unavailable".into()
            } else if conflict(&error) {
                clean_error(&error)
            } else {
                error
            });
            attention::refresh(model)
        }
        Request::TaskVerb { .. } => {
            model.set_sticky_toast(error);
            attention::refresh(model)
        }
        Request::RunControl { workspace, run, .. } | Request::RunApprove { workspace, run, .. } => {
            model.set_sticky_toast(clean_error(&error));
            refetch_run(model, workspace, run)
        }
        Request::Sessions { .. } => {
            model.set_sticky_toast(error);
            Vec::new()
        }
        _ => {
            model.set_sticky_toast(error);
            Vec::new()
        }
    }
}

fn unavailable(error: &str) -> bool {
    error.contains("503")
        || error.to_lowercase().contains("unavailable")
        || error.to_lowercase().contains("draining")
}

fn conflict(error: &str) -> bool {
    error.contains("409") || error.to_lowercase().contains("already")
}

fn not_found(error: &str) -> bool {
    error.contains("404") || error.to_lowercase().contains("not found")
}

fn apply(model: &mut Model, request: Request, response: ApiResponse) -> Vec<Cmd> {
    match (request, response) {
        (Request::Status { .. }, ApiResponse::Status(status)) => {
            model.daemon.status = status.daemon.status;
            model.daemon.version = status.daemon.version;
            model.dirty = true;
            Vec::new()
        }
        (Request::Workspaces { .. }, ApiResponse::Workspaces(items)) => {
            if let Some(crate::app::model::Overlay::WorkspacePicker {
                items: target,
                selected,
                ..
            }) = &mut model.overlay
            {
                *target = items
                    .into_iter()
                    .map(|item| WorkspaceRef {
                        id: item.id,
                        name: item.name,
                        root_dir: item.root_dir,
                    })
                    .collect();
                *selected = (!target.is_empty()).then_some(0);
            }
            model.dirty = true;
            Vec::new()
        }
        (Request::Sessions { .. }, ApiResponse::Sessions(page)) => {
            sessions::apply(model, *page);
            attention::rebuild(model);
            model.dirty = true;
            let mut commands = attention::refresh(model);
            commands.extend(super::detail_session::reopen_if_active(model));
            commands
        }
        (Request::Runs { .. }, ApiResponse::Runs(page)) => {
            runs::apply(model, *page);
            model.dirty = true;
            runs::any_live(model)
                .then(|| Cmd::After(Duration::from_secs(5), TimerId::RunsPoll))
                .into_iter()
                .collect()
        }
        (Request::Overview { .. }, ApiResponse::Overview(overview)) => {
            model.attention_overview_total = overview.attention.total;
            model.attention_overview = overview.attention.items;
            model.attention_overview_unavailable = false;
            attention::rebuild(model);
            model.dirty = true;
            Vec::new()
        }
        (Request::Clarifications { session, .. }, ApiResponse::Clarifications(items)) => {
            model.attention_clarifications.insert(session, items);
            attention::rebuild(model);
            model.dirty = true;
            Vec::new()
        }
        (Request::VisibleTranscript { session, .. }, ApiResponse::TranscriptPage(page)) => {
            attention::apply_transcript(model, &session, &page);
            model.dirty = true;
            Vec::new()
        }
        (Request::Session { session: id, .. }, ApiResponse::Session(session)) => {
            if let Some(detail) = model
                .session_detail_mut()
                .filter(|detail| detail.session.id == id)
            {
                detail.session = session;
            } else {
                model.detail = Detail::Session(Box::new(SessionDetail::new(session)));
            }
            model.focus = Panel::Detail;
            model.dirty = true;
            Vec::new()
        }
        (Request::TranscriptPage { session, .. }, ApiResponse::TranscriptPage(page)) => {
            attention::apply_transcript(model, &session, &page);
            let Some(detail) = model
                .session_detail_mut()
                .filter(|detail| detail.session.id == session)
            else {
                return Vec::new();
            };
            page_into_detail(detail, *page);
            let cursor = detail.transcript.cursor();
            let stopped = detail.view.stopped;
            model.stream_cursors.insert(
                StreamId::Transcript(session.clone()),
                format!(
                    "{}:{}:{}",
                    cursor.epoch, cursor.generation, cursor.after_sequence
                ),
            );
            model
                .active_streams
                .insert(StreamId::Transcript(session.clone()));
            model.dirty = true;
            if stopped {
                model.active_streams.remove(&StreamId::Transcript(session));
                Vec::new()
            } else {
                vec![Cmd::StartStream(StreamId::Transcript(session))]
            }
        }
        (Request::Run { run, .. }, ApiResponse::Run(value)) => {
            if let Detail::Run(detail) = &mut model.detail
                && detail.run_id == run
            {
                detail.run = Some(*value);
                detail.stream = StreamStatus::Live;
                model.dirty = true;
            }
            Vec::new()
        }
        (Request::Prompt { session, .. }, ApiResponse::Prompt(outcome)) => {
            model.prompt_pending = false;
            model.failed_prompt = None;
            model.app_created_sessions.remove(&session);
            let stopped = model
                .session_detail()
                .filter(|detail| detail.session.id == session)
                .is_some_and(|detail| {
                    detail.view.stopped || detail.stream == StreamStatus::Stopped
                });
            if let Some(detail) = model
                .session_detail_mut()
                .filter(|detail| detail.session.id == session)
            {
                detail.composer.clear();
                detail.view.stopped = false;
                detail.stream = StreamStatus::Live;
                detail.session.state = "active".into();
            }
            let text = match outcome {
                PromptOutcome::Sent => "sent".into(),
                PromptOutcome::Accepted(result) => match result.delivery.as_str() {
                    "steer" => "steering".into(),
                    "interrupt_then_prompt" => "interrupting".into(),
                    _ => result.queue_position.map_or_else(
                        || "queued".into(),
                        |position| format!("queued · position {position}"),
                    ),
                },
            };
            let mut commands = model.set_success_toast(text);
            if stopped {
                let stream = StreamId::Transcript(session);
                model.active_streams.insert(stream.clone());
                commands.push(Cmd::StartStream(stream));
            }
            commands
        }
        (Request::CreateSession { .. }, ApiResponse::SessionCreated(session)) => {
            created_session(model, session)
        }
        (Request::Approve { request, .. }, ApiResponse::Empty) => {
            let text = match request.decision {
                Decision::AllowOnce | Decision::AllowAlways => "allowed",
                Decision::RejectOnce | Decision::RejectAlways => "rejected",
            };
            let mut commands = model.set_success_toast(text);
            commands.extend(attention::refresh(model));
            commands
        }
        (Request::AnswerClarification { .. }, ApiResponse::ClarificationAnswered(_)) => {
            model.overlay = None;
            let mut commands = model.set_success_toast("answered");
            commands.extend(attention::refresh(model));
            commands
        }
        (Request::TaskVerb { verb, .. }, ApiResponse::Empty) => {
            let text = match verb {
                TaskVerb::Approve => "approved",
                TaskVerb::Reject => "rejected",
                TaskVerb::Retry => "retry requested",
            };
            let mut commands = model.set_success_toast(text);
            commands.extend(attention::refresh(model));
            commands
        }
        (Request::CancelPrompt { .. }, ApiResponse::Empty) => {
            model.set_success_toast("cancel requested")
        }
        (_, _) => Vec::new(),
    }
}

fn clean_error(error: &str) -> String {
    error
        .strip_prefix("HTTP ")
        .and_then(|rest| rest.split_once(": ").map(|(_, message)| message))
        .unwrap_or(error)
        .to_owned()
}

fn prompt_failure(error: &str) -> String {
    let lower = error.to_lowercase();
    if lower.contains("transport:") || lower.contains("timeout") {
        return "request failed — check the session before retrying".into();
    }
    if lower.contains("413") {
        let cap = lower
            .split("queue_cap=")
            .nth(1)
            .and_then(|tail| tail.split(|c: char| !c.is_ascii_digit()).next())
            .filter(|value| !value.is_empty())
            .unwrap_or("?");
        return format!("input queue full (cap {cap})");
    }
    if lower.contains("503") || lower.contains("draining") {
        return "daemon is draining — writes refused".into();
    }
    if lower.contains("409")
        && (lower.contains("active turn mismatch") || lower.contains("active_turn_mismatch"))
    {
        return "turn changed — check the session and send again".into();
    }
    if lower.contains("409") {
        return format!(
            "check the session — indeterminate dispatch ({})",
            clean_error(error)
        );
    }
    clean_error(error)
}

fn refetch_run(model: &mut Model, workspace: &str, run: &str) -> Vec<Cmd> {
    let mut commands = runs::request(model).into_iter().collect::<Vec<_>>();
    let request = model.allocate(|id| Request::Run {
        id,
        workspace: workspace.to_owned(),
        run: run.to_owned(),
    });
    commands.push(Cmd::Get(request));
    commands
}

fn created_session(model: &mut Model, session: compozy_client::types::Session) -> Vec<Cmd> {
    let Some(workspace) = model.workspace.as_ref().map(|value| value.id.clone()) else {
        return Vec::new();
    };
    model.create_session_pending = false;
    model.app_created_sessions.insert(session.id.clone());
    if !model
        .sessions_unfiltered
        .iter()
        .any(|row| row.id == session.id)
    {
        model.sessions_unfiltered.insert(0, (&session).into());
    }
    let new_id = session.id.clone();
    sessions::refilter(model, Some(&new_id));
    model.sessions.selected = model.sessions.items.iter().position(|row| row.id == new_id);
    let mut detail = SessionDetail::new(session);
    detail.composer.focused = true;
    model.detail = Detail::Session(Box::new(detail));
    model.focus = Panel::Detail;
    model
        .active_streams
        .insert(StreamId::Transcript(new_id.clone()));
    let mut commands = model.set_success_toast("session created");
    if let Some(command) = sessions::request(model) {
        commands.push(command);
    }
    let transcript = model.allocate(|id| Request::TranscriptPage {
        id,
        workspace: workspace.clone(),
        session: new_id.clone(),
        before_sequence: None,
    });
    let clarifications = model.allocate(|id| Request::Clarifications {
        id,
        workspace,
        session: new_id.clone(),
    });
    commands.extend([
        Cmd::Get(transcript),
        Cmd::Get(clarifications),
        Cmd::StartStream(StreamId::Transcript(new_id)),
    ]);
    commands
}
