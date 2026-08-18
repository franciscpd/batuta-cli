use crate::{
    app::{Detail, Model, Overlay, Panel},
    cmd::{Cmd, LogScope, Request, StreamId},
};
use compozy_client::types::LogEvent;
use crossterm::event::{KeyCode, KeyEvent};

pub(super) fn open(model: &mut Model) -> Vec<Cmd> {
    let scope = focused_scope(model);
    model.overlay = Some(Overlay::Logs {
        scope: scope.clone(),
        error_only: false,
        events: Vec::new(),
        selection: 0,
        cursor_reset: false,
    });
    start(model, scope, false)
}

fn focused_scope(model: &Model) -> LogScope {
    match model.focus {
        Panel::Detail => match &model.detail {
            Detail::Session(detail) => LogScope::Session(detail.session.id.clone()),
            Detail::Run(detail) => LogScope::Run(detail.run_id.clone()),
            Detail::Empty => LogScope::Workspace,
        },
        Panel::Sessions => model
            .sessions
            .selected()
            .map_or(LogScope::Workspace, |row| LogScope::Session(row.id.clone())),
        Panel::Runs => model
            .runs
            .selected()
            .map_or(LogScope::Workspace, |row| LogScope::Run(row.id.clone())),
        Panel::Attention => model
            .attention
            .get(model.attention_selected.unwrap_or_default())
            .map_or(LogScope::Workspace, |item| {
                item.session_id
                    .clone()
                    .map(LogScope::Session)
                    .or_else(|| item.run_id.clone().map(LogScope::Run))
                    .unwrap_or(LogScope::Workspace)
            }),
    }
}

fn start(model: &mut Model, scope: LogScope, error_only: bool) -> Vec<Cmd> {
    let Some(workspace) = model.workspace.as_ref().map(|value| value.id.clone()) else {
        return Vec::new();
    };
    let request = model.allocate(|id| Request::Logs {
        id,
        workspace,
        scope: scope.clone(),
        error_only,
        limit: 200,
    });
    let stream = StreamId::Logs(scope);
    model.active_streams.insert(stream.clone());
    model.dirty = true;
    vec![Cmd::Get(request), Cmd::StartStream(stream)]
}

pub(super) fn key(model: &mut Model, key: KeyEvent) -> Vec<Cmd> {
    let Some(Overlay::Logs {
        scope,
        error_only,
        events,
        selection,
        ..
    }) = &mut model.overlay
    else {
        return Vec::new();
    };
    match key.code {
        KeyCode::Esc | KeyCode::Char('L') => {
            let stream = StreamId::Logs(scope.clone());
            model.active_streams.remove(&stream);
            model.overlay = None;
            model.dirty = true;
            vec![Cmd::StopStream(stream)]
        }
        KeyCode::Char('e') => {
            let scope = scope.clone();
            *error_only = !*error_only;
            let enabled = *error_only;
            events.clear();
            *selection = 0;
            model.stream_cursors.remove(&StreamId::Logs(scope.clone()));
            let mut commands = vec![Cmd::StopStream(StreamId::Logs(scope.clone()))];
            commands.extend(start(model, scope, enabled));
            commands
        }
        KeyCode::Char('j') | KeyCode::Down => {
            *selection = (*selection + 1).min(events.len().saturating_sub(1));
            model.dirty = true;
            Vec::new()
        }
        KeyCode::Char('k') | KeyCode::Up => {
            *selection = selection.saturating_sub(1);
            model.dirty = true;
            Vec::new()
        }
        KeyCode::PageDown => {
            *selection = (*selection + 10).min(events.len().saturating_sub(1));
            model.dirty = true;
            Vec::new()
        }
        KeyCode::PageUp => {
            *selection = selection.saturating_sub(10);
            model.dirty = true;
            Vec::new()
        }
        _ => Vec::new(),
    }
}

pub(super) fn apply(model: &mut Model, scope: &LogScope, events: Vec<LogEvent>) {
    if let Some(Overlay::Logs {
        scope: active,
        events: target,
        selection,
        ..
    }) = &mut model.overlay
        && active == scope
    {
        *target = events;
        *selection = target.len().saturating_sub(1);
        model.dirty = true;
    }
}

pub(super) fn push(model: &mut Model, scope: &LogScope, event: LogEvent) {
    if let Some(Overlay::Logs {
        scope: active,
        events,
        selection,
        ..
    }) = &mut model.overlay
        && active == scope
    {
        events.push(event);
        if events.len() > 200 {
            events.remove(0);
        }
        *selection = events.len().saturating_sub(1);
    }
}

pub(super) fn cursor_reset(model: &mut Model, scope: LogScope) -> Vec<Cmd> {
    let Some(Overlay::Logs {
        scope: active,
        error_only,
        events,
        cursor_reset,
        selection,
    }) = &mut model.overlay
    else {
        return Vec::new();
    };
    if *active != scope {
        return Vec::new();
    }
    let enabled = *error_only;
    events.clear();
    *selection = 0;
    *cursor_reset = true;
    model.stream_cursors.remove(&StreamId::Logs(scope.clone()));
    let Some(workspace) = model.workspace.as_ref().map(|value| value.id.clone()) else {
        return Vec::new();
    };
    let request_scope = scope.clone();
    let request = model.allocate(|id| Request::Logs {
        id,
        workspace,
        scope: request_scope,
        error_only: enabled,
        limit: 200,
    });
    model.dirty = true;
    vec![Cmd::Get(request), Cmd::StartStream(StreamId::Logs(scope))]
}
