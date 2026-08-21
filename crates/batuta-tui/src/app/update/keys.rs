use crate::{
    app::model::{
        Detail, FooterState, Model, Overlay, Panel, StreamStatus, entry_has_expandable_part,
    },
    cmd::{Cmd, Request, StreamId, TimerId},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::Duration;

pub(super) fn key(model: &mut Model, key: KeyEvent) -> Vec<Cmd> {
    if key.kind != KeyEventKind::Press {
        return Vec::new();
    }
    if let Some(toast) = &model.toast
        && toast.sticky
    {
        model.toast = None;
        if key.code == KeyCode::Esc {
            model.dirty = true;
            return Vec::new();
        }
    }
    let ctrl_c = key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c');
    if model.too_small() {
        return if ctrl_c || key.code == KeyCode::Char('q') {
            quit(model)
        } else {
            Vec::new()
        };
    }
    if ctrl_c {
        return guarded_quit(model);
    }
    if model.overlay.is_some() {
        return overlay_key(model, key);
    }
    if key.code == KeyCode::F(1) {
        model.overlay = Some(Overlay::Help { scroll: 0 });
        model.dirty = true;
        return Vec::new();
    }
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && key.code == KeyCode::Char('x')
        && model.focus == Panel::Detail
        && model.session_detail().is_some()
    {
        return super::detail_session::cancel_key(model);
    }
    if chooser_or_confirm(model) {
        return chooser_key(model, key);
    }
    if model.text_field_focused() {
        return text_key(model, key);
    }
    if key.code == KeyCode::F(1) || key.code == KeyCode::Char('?') {
        model.overlay = Some(Overlay::Help { scroll: 0 });
        model.dirty = true;
        return Vec::new();
    }
    if key.code == KeyCode::Char('q') {
        return guarded_quit(model);
    }
    if key.code == KeyCode::Char('L') {
        return super::logs::open(model);
    }
    if model.mode == crate::app::model::AppMode::TailOnly {
        return detail_key(model, key);
    }
    match key.code {
        KeyCode::Tab => {
            model.focus = if key.modifiers.contains(KeyModifiers::SHIFT) {
                model.focus.previous()
            } else {
                model.focus.next()
            };
            focus_changed(model)
        }
        KeyCode::BackTab => {
            model.focus = model.focus.previous();
            focus_changed(model)
        }
        KeyCode::Char('1' | '2' | '3' | '4') => {
            model.focus = match key.code {
                KeyCode::Char('1') => Panel::Sessions,
                KeyCode::Char('2') => Panel::Runs,
                KeyCode::Char('3') => Panel::Attention,
                _ => Panel::Detail,
            };
            focus_changed(model)
        }
        KeyCode::Char('w') => super::picker::open(model, false),
        _ => match model.focus {
            Panel::Sessions | Panel::Runs | Panel::Attention => list_key(model, key),
            Panel::Detail => detail_key(model, key),
        },
    }
}

fn focus_changed(model: &mut Model) -> Vec<Cmd> {
    model.dirty = true;
    Vec::new()
}

fn list_key(model: &mut Model, key: KeyEvent) -> Vec<Cmd> {
    if model.focus == Panel::Attention {
        return super::attention::key(model, key);
    }
    match model.focus {
        Panel::Sessions => match key.code {
            KeyCode::Char('j') | KeyCode::Down | KeyCode::PageDown | KeyCode::Char('G') => {
                model.sessions.select_next()
            }
            KeyCode::Char('k') | KeyCode::Up | KeyCode::PageUp | KeyCode::Char('g') => {
                model.sessions.select_previous()
            }
            KeyCode::Char('/') => model.sessions.filter_focused = true,
            KeyCode::Char('*') => return super::sessions::toggle_agent(model),
            KeyCode::Char('r') => return super::sessions::refresh(model),
            KeyCode::Char('n') => return super::sessions::create(model),
            KeyCode::Enter => return open_session(model),
            _ => return Vec::new(),
        },
        Panel::Runs => match key.code {
            KeyCode::Char('j') | KeyCode::Down | KeyCode::PageDown | KeyCode::Char('G') => {
                model.runs.select_next()
            }
            KeyCode::Char('k') | KeyCode::Up | KeyCode::PageUp | KeyCode::Char('g') => {
                model.runs.select_previous()
            }
            KeyCode::Char('/') => model.runs.filter_focused = true,
            KeyCode::Char('*') => return super::runs::toggle_loop(model),
            KeyCode::Char('r') => return super::runs::refresh(model),
            KeyCode::Enter => return open_run(model),
            _ => return Vec::new(),
        },
        Panel::Attention => {}
        Panel::Detail => return Vec::new(),
    }
    model.dirty = true;
    Vec::new()
}

pub(super) fn open_session(model: &mut Model) -> Vec<Cmd> {
    let Some(row) = model.sessions.selected().cloned() else {
        return Vec::new();
    };
    open_session_id(model, row.id)
}

pub(super) fn open_session_id(model: &mut Model, id: String) -> Vec<Cmd> {
    let Some(workspace) = model.workspace.clone() else {
        return Vec::new();
    };
    let row = model
        .sessions
        .items
        .iter()
        .find(|row| row.id == id)
        .cloned();
    let old = match &model.detail {
        Detail::Session(detail) => Some(detail.session.id.clone()),
        _ => None,
    };
    let old_run = match &model.detail {
        Detail::Run(detail) => Some(detail.run_id.clone()),
        _ => None,
    };
    let session = compozy_client::types::Session {
        id: id.clone(),
        agent_name: row
            .as_ref()
            .map_or_else(String::new, |row| row.agent.clone()),
        name: row.as_ref().and_then(|row| row.name.clone()),
        state: row
            .as_ref()
            .map_or_else(String::new, |row| row.state.clone()),
        ..Default::default()
    };
    model.detail = Detail::Session(Box::new(crate::app::model::SessionDetail::new(session)));
    model.focus = Panel::Detail;
    let detail_request = model.allocate(|request_id| Request::Session {
        id: request_id,
        workspace: workspace.id.clone(),
        session: id.clone(),
    });
    let page_request = model.allocate(|request_id| Request::TranscriptPage {
        id: request_id,
        workspace: workspace.id.clone(),
        session: id.clone(),
        before_sequence: None,
    });
    let clarification_request = model.allocate(|request_id| Request::Clarifications {
        id: request_id,
        workspace: workspace.id.clone(),
        session: id.clone(),
    });
    let mut commands = Vec::new();
    if let Some(old) = old.filter(|old| old != &id) {
        model
            .active_streams
            .remove(&StreamId::Transcript(old.clone()));
        commands.push(Cmd::StopStream(StreamId::Transcript(old)));
    }
    if let Some(old) = old_run {
        model
            .active_streams
            .remove(&StreamId::RunEvents(old.clone()));
        commands.push(Cmd::StopStream(StreamId::RunEvents(old)));
    }
    model
        .active_streams
        .insert(StreamId::Transcript(id.clone()));
    commands.extend([
        Cmd::Get(detail_request),
        Cmd::Get(page_request),
        Cmd::StartStream(StreamId::Transcript(id)),
        Cmd::Get(clarification_request),
    ]);
    model.dirty = true;
    commands
}
pub(super) fn open_run(model: &mut Model) -> Vec<Cmd> {
    let Some(row) = model.runs.selected().cloned() else {
        return Vec::new();
    };
    open_run_id(model, row.id)
}

pub(super) fn open_run_id(model: &mut Model, id: String) -> Vec<Cmd> {
    let Some(workspace) = model.workspace.clone() else {
        return Vec::new();
    };
    let old_transcript = match &model.detail {
        Detail::Session(detail) => Some(detail.session.id.clone()),
        _ => None,
    };
    let old_run = match &model.detail {
        Detail::Run(detail) if detail.run_id != id => Some(detail.run_id.clone()),
        _ => None,
    };
    model.detail = Detail::Run(Box::new(crate::app::model::RunDetail {
        run: None,
        run_id: id.clone(),
        events: Vec::new(),
        selection: 0,
        confirm: None,
        stream: StreamStatus::Live,
    }));
    model.focus = Panel::Detail;
    model.active_streams.insert(StreamId::RunEvents(id.clone()));
    let request = model.allocate(|request_id| Request::Run {
        id: request_id,
        workspace: workspace.id,
        run: id.clone(),
    });
    model.dirty = true;
    let mut commands = Vec::new();
    if let Some(old) = old_transcript {
        model
            .active_streams
            .remove(&StreamId::Transcript(old.clone()));
        commands.push(Cmd::StopStream(StreamId::Transcript(old)));
    }
    if let Some(old) = old_run {
        model
            .active_streams
            .remove(&StreamId::RunEvents(old.clone()));
        commands.push(Cmd::StopStream(StreamId::RunEvents(old)));
    }
    commands.extend([Cmd::Get(request), Cmd::StartStream(StreamId::RunEvents(id))]);
    commands
}

fn detail_key(model: &mut Model, key: KeyEvent) -> Vec<Cmd> {
    if matches!(model.detail, Detail::Run(_)) {
        return super::detail_run::key(model, key);
    }
    if let Some(commands) = super::attention::inline_key(model, key) {
        return commands;
    }
    if matches!(key.code, KeyCode::Char('k') | KeyCode::Up) {
        return move_up(model, 1);
    }
    if key.code == KeyCode::PageUp {
        let amount = model
            .session_detail()
            .map_or(1, |detail| detail.view.visible_height);
        return move_up(model, amount);
    }
    let Some(detail) = model.session_detail_mut() else {
        return Vec::new();
    };
    let rows = detail.transcript.presentation_rows(detail.view.raw_debug);
    let len = rows.len();
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if len > 0 {
                detail.view.selection = (detail.view.selection + 1).min(len - 1);
                detail.view.follow = detail.view.selection + 1 == len;
            }
        }
        KeyCode::PageDown => {
            detail.view.selection =
                (detail.view.selection + detail.view.visible_height).min(len.saturating_sub(1));
        }
        KeyCode::Char('G') => {
            detail.view.selection = len.saturating_sub(1);
            detail.view.follow = true;
            detail.view.footer = FooterState::Live;
        }
        KeyCode::Char('g') => {
            detail.view.selection = 0;
            detail.view.follow = false;
        }
        KeyCode::Char('t') => {
            detail.view.reasoning_expanded = !detail.view.reasoning_expanded;
            detail.view.cache_dirty = true;
        }
        KeyCode::Char('D') => {
            let entries = detail.transcript.entries();
            let selected_row_start_sequence = rows
                .get(detail.view.selection)
                .and_then(|row| entries.get(row.first_entry_index()))
                .map(|entry| entry.start_sequence);
            let row_contains_start_sequence =
                |row: &crate::transcript::PresentationRow, start_sequence| match row {
                    crate::transcript::PresentationRow::Entry { entry_index } => entries
                        .get(*entry_index)
                        .is_some_and(|entry| entry.start_sequence == start_sequence),
                    crate::transcript::PresentationRow::Group { entry_indexes, .. } => {
                        entry_indexes.iter().any(|entry_index| {
                            entries
                                .get(*entry_index)
                                .is_some_and(|entry| entry.start_sequence == start_sequence)
                        })
                    }
                };
            let selected_start_sequence = detail
                .view
                .selected_source_start_sequence
                .filter(|start_sequence| {
                    rows.get(detail.view.selection)
                        .is_some_and(|row| row_contains_start_sequence(row, *start_sequence))
                })
                .or(selected_row_start_sequence);
            detail.view.selected_source_start_sequence = selected_start_sequence;
            detail.view.raw_debug = !detail.view.raw_debug;
            let rows = detail.transcript.presentation_rows(detail.view.raw_debug);
            detail.view.selection = selected_start_sequence
                .and_then(|start_sequence| {
                    rows.iter()
                        .position(|row| row_contains_start_sequence(row, start_sequence))
                })
                .unwrap_or_else(|| detail.view.selection.min(rows.len().saturating_sub(1)));
            detail.view.cache_dirty = true;
        }
        KeyCode::Char('i') => detail.composer.focused = true,
        KeyCode::Enter => {
            let selected_start = rows.get(detail.view.selection).and_then(|row| {
                detail
                    .transcript
                    .entries()
                    .get(row.first_entry_index())
                    .map(|entry| entry.start_sequence)
            });
            if let Some(entry) = selected_start
                .and_then(|start_sequence| detail.transcript.entry(start_sequence))
                .filter(|entry| entry_has_expandable_part(entry))
            {
                if !detail.view.expanded.remove(&entry.start_sequence) {
                    detail.view.expanded.insert(entry.start_sequence);
                }
                detail.view.cache_dirty = true;
            } else {
                detail.composer.focused = true;
            }
        }
        KeyCode::Esc => return Vec::new(),
        _ => return Vec::new(),
    }
    model.dirty = true;
    Vec::new()
}

fn move_up(model: &mut Model, amount: usize) -> Vec<Cmd> {
    let Some(detail) = model.session_detail_mut() else {
        return Vec::new();
    };
    if detail.view.selection > 0 {
        detail.view.selection = detail.view.selection.saturating_sub(amount);
        detail.view.follow = false;
        model.dirty = true;
        return Vec::new();
    }
    if !detail.transcript.has_older {
        detail.view.beginning = true;
        model.dirty = true;
        return Vec::new();
    }
    if detail.view.fetching {
        return Vec::new();
    }
    detail.view.fetching = true;
    let session = detail.session.id.clone();
    let before_sequence = detail.transcript.next_before_sequence;
    let Some(workspace) = model.workspace.as_ref().map(|value| value.id.clone()) else {
        return Vec::new();
    };
    let request = model.allocate(|id| Request::TranscriptPage {
        id,
        workspace,
        session,
        before_sequence,
    });
    vec![Cmd::Get(request)]
}

fn text_key(model: &mut Model, key: KeyEvent) -> Vec<Cmd> {
    if matches!(model.overlay, Some(Overlay::Clarify { .. })) {
        return super::clarify::key(model, key);
    }
    if model.sessions.filter_focused || model.runs.filter_focused {
        let filtering_sessions = model.sessions.filter_focused;
        let (filter, focused) = if filtering_sessions {
            (
                &mut model.sessions.filter,
                &mut model.sessions.filter_focused,
            )
        } else {
            (&mut model.runs.filter, &mut model.runs.filter_focused)
        };
        match key.code {
            KeyCode::Esc => {
                filter.clear();
                *focused = false
            }
            KeyCode::Enter => *focused = false,
            KeyCode::Char(c) => filter.push(c),
            KeyCode::Backspace => {
                filter.pop();
            }
            _ => {}
        }
        if filtering_sessions {
            crate::app::panels::sessions::refilter(model, None);
            crate::app::panels::attention::rebuild(model);
        } else {
            crate::app::panels::runs::refilter(model, None);
        }
        model.dirty = true;
        return Vec::new();
    }
    super::composer::key(model, key)
}

fn overlay_key(model: &mut Model, key: KeyEvent) -> Vec<Cmd> {
    if matches!(model.overlay, Some(Overlay::Clarify { .. })) {
        return super::clarify::key(model, key);
    }
    if matches!(model.overlay, Some(Overlay::Logs { .. })) {
        return super::logs::key(model, key);
    }
    if matches!(model.overlay, Some(Overlay::WorkspacePicker { .. })) {
        return super::picker::key(model, key);
    }
    if matches!(model.overlay, Some(Overlay::WorkspaceOnboarding { .. })) {
        return onboarding_key(model, key);
    }
    match &mut model.overlay {
        Some(Overlay::Help { scroll }) => match key.code {
            KeyCode::Esc | KeyCode::Char('?') => model.overlay = None,
            KeyCode::Char('j') | KeyCode::Down => *scroll = scroll.saturating_add(1),
            KeyCode::Char('k') | KeyCode::Up => *scroll = scroll.saturating_sub(1),
            _ => return Vec::new(),
        },
        Some(Overlay::Clarify { .. } | Overlay::WorkspaceOnboarding { .. }) => return Vec::new(),
        _ => return Vec::new(),
    }
    model.dirty = true;
    Vec::new()
}

fn onboarding_key(model: &mut Model, key: KeyEvent) -> Vec<Cmd> {
    let Some(Overlay::WorkspaceOnboarding {
        candidate,
        confirming,
        adding,
        booting,
        refresh_required,
        registration_complete,
        registration_unsupported,
        message,
        details_expanded,
        ..
    }) = &mut model.overlay
    else {
        return Vec::new();
    };
    if *adding || *booting {
        return Vec::new();
    }
    if (*refresh_required || *registration_complete)
        && !matches!(key.code, KeyCode::Char('w' | 'q' | 'r') | KeyCode::Esc)
    {
        return Vec::new();
    }
    match key.code {
        KeyCode::Char('w') => super::picker::open(model, false),
        KeyCode::Char('q') => quit(model),
        KeyCode::Char('r') => {
            if !*registration_unsupported {
                *message = Some("refreshing workspace catalog…".into());
            }
            let request = model.allocate(|id| Request::Workspaces { id });
            model.dirty = true;
            vec![Cmd::Get(request)]
        }
        KeyCode::Esc if *confirming => {
            *confirming = false;
            model.dirty = true;
            Vec::new()
        }
        KeyCode::Char('a') if !*confirming => {
            *confirming = true;
            *message = None;
            *details_expanded = false;
            model.dirty = true;
            Vec::new()
        }
        KeyCode::Char('d') => {
            *details_expanded = !*details_expanded;
            model.dirty = true;
            Vec::new()
        }
        KeyCode::Enter if *confirming => {
            *adding = true;
            *message = None;
            let name = candidate.name.clone();
            let root_dir = candidate.root_dir.clone();
            let request = model.allocate(|id| Request::AddWorkspace { id, name, root_dir });
            model.dirty = true;
            vec![Cmd::Post(request)]
        }
        _ => Vec::new(),
    }
}

fn chooser_or_confirm(model: &Model) -> bool {
    if model.attention_confirm.is_some() || model.inline_permission_confirm.is_some() {
        return true;
    }
    match &model.detail {
        Detail::Session(detail) => detail.chooser.is_some() || detail.confirm.is_some(),
        Detail::Run(detail) => detail.confirm.is_some(),
        Detail::Empty => false,
    }
}
fn chooser_key(model: &mut Model, key: KeyEvent) -> Vec<Cmd> {
    if model.attention_confirm.is_some() {
        return match key.code {
            KeyCode::Enter => super::attention::confirm(model),
            KeyCode::Esc => {
                super::attention::cancel_confirm(model);
                Vec::new()
            }
            _ => Vec::new(),
        };
    }
    if model.inline_permission_confirm.is_some() {
        return match key.code {
            KeyCode::Enter => super::attention::confirm_inline(model),
            KeyCode::Esc => {
                super::attention::cancel_confirm(model);
                Vec::new()
            }
            _ => Vec::new(),
        };
    }
    if model
        .session_detail()
        .is_some_and(|detail| detail.chooser.is_some())
    {
        return super::prompt::chooser_key(model, key);
    }
    if model.session_detail().is_some_and(|detail| {
        detail
            .confirm
            .as_ref()
            .is_some_and(|confirm| confirm.prompt.starts_with("cancel the current turn"))
    }) {
        return match key.code {
            KeyCode::Enter => super::detail_session::confirm_cancel(model),
            KeyCode::Esc => {
                model.session_detail_mut().unwrap().confirm = None;
                model.dirty = true;
                Vec::new()
            }
            _ => Vec::new(),
        };
    }
    if matches!(model.detail, Detail::Run(ref detail) if detail.confirm.is_some()) {
        return super::detail_run::confirm(model, key.code);
    }
    if key.code == KeyCode::Esc {
        if let Some(detail) = model.session_detail_mut() {
            detail.chooser = None;
            detail.confirm = None;
        }
        if let Detail::Run(detail) = &mut model.detail {
            detail.confirm = None;
        }
        model.dirty = true;
    }
    Vec::new()
}

fn guarded_quit(model: &mut Model) -> Vec<Cmd> {
    if model.draft_nonempty() && !model.quit_guard {
        model.quit_guard = true;
        model.set_sticky_toast("press Ctrl+C again to quit — draft will be lost");
        vec![Cmd::After(Duration::from_secs(3), TimerId::QuitGuard)]
    } else {
        quit(model)
    }
}
pub(super) fn quit(model: &mut Model) -> Vec<Cmd> {
    let mut commands = model.all_stop_cmds();
    commands.push(Cmd::Quit);
    commands
}
