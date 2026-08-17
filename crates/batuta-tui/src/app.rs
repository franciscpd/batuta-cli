use crate::{
    cmd::Cmd,
    msg::Msg,
    theme::Theme,
    transcript::{Applied, TranscriptState},
};
use compozy_client::{
    TranscriptEvent,
    types::{Entry, Part, TranscriptDelta, TranscriptSnapshot},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::text::Text;
use std::collections::{BTreeSet, HashMap};
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct SessionHeader {
    pub workspace: String,
    pub workspace_id: String,
    pub session_id: String,
    pub agent: String,
    pub name: Option<String>,
    pub state: String,
    pub warning: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FooterState {
    Live,
    Stopped {
        reason: Option<String>,
        detail: Option<String>,
    },
    Reconnecting(u32),
    Offline,
    Resynchronized(String),
    NewBelow(usize),
    Fatal(String),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct RenderCacheKey {
    pub start_sequence: i64,
    pub sequence: i64,
    pub width: u16,
    pub reasoning_expanded: bool,
    pub expanded: bool,
    pub color: bool,
}

#[derive(Clone, Debug)]
pub struct Model {
    pub header: SessionHeader,
    pub transcript: TranscriptState,
    pub selection: usize,
    pub reasoning_expanded: bool,
    pub expanded: BTreeSet<i64>,
    pub follow: bool,
    pub footer: FooterState,
    pub size: (u16, u16),
    pub visible_height: usize,
    pub warning: Option<String>,
    pub dirty: bool,
    cache_dirty: bool,
    pub fetching: bool,
    pub stopped: bool,
    pub beginning: bool,
    pub theme: Theme,
    pub render_cache: HashMap<RenderCacheKey, Text<'static>>,
}

impl Model {
    pub fn new(header: SessionHeader) -> Self {
        let stopped = header.state == "stopped";
        Self {
            warning: header.warning.clone(),
            footer: if stopped {
                FooterState::Stopped {
                    reason: None,
                    detail: None,
                }
            } else {
                FooterState::Live
            },
            header,
            transcript: TranscriptState::default(),
            selection: 0,
            reasoning_expanded: false,
            expanded: BTreeSet::new(),
            follow: true,
            size: (80, 24),
            visible_height: 20,
            dirty: true,
            cache_dirty: true,
            fetching: true,
            stopped,
            beginning: false,
            theme: Theme::detect(),
            render_cache: HashMap::new(),
        }
    }

    pub fn initial_cmds(&self) -> Vec<Cmd> {
        vec![Cmd::FetchPage {
            before_sequence: None,
        }]
    }
}

pub fn update(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    let commands = update_inner(model, msg);
    if model.cache_dirty {
        crate::render_cache::rebuild(model);
        model.cache_dirty = false;
    }
    commands
}

fn update_inner(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    match msg {
        Msg::Key(key) => update_key(model, key),
        Msg::Mouse(_) => Vec::new(),
        Msg::Resize(width, height) => {
            model.size = (width, height);
            model.visible_height = usize::from(height.saturating_sub(4)).max(1);
            model.dirty = true;
            model.cache_dirty = true;
            Vec::new()
        }
        Msg::Tick => {
            if model.dirty {
                model.dirty = false;
                vec![Cmd::Render]
            } else {
                Vec::new()
            }
        }
        Msg::Page(result) => apply_page(model, result),
        Msg::Stream(event) => apply_stream(model, event),
        Msg::ClearFooter => {
            if matches!(model.footer, FooterState::Resynchronized(_)) {
                model.footer = FooterState::Live;
                model.dirty = true;
            }
            Vec::new()
        }
        Msg::Quit => vec![Cmd::StopStream, Cmd::Quit],
    }
}

fn update_key(model: &mut Model, key: KeyEvent) -> Vec<Cmd> {
    if key.kind != KeyEventKind::Press {
        return Vec::new();
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return vec![Cmd::StopStream, Cmd::Quit];
    }
    let len = model.transcript.len();
    match key.code {
        KeyCode::Char('q') => return vec![Cmd::StopStream, Cmd::Quit],
        KeyCode::Esc => return Vec::new(),
        KeyCode::Char('j') | KeyCode::Down => {
            if len > 0 {
                model.selection = (model.selection + 1).min(len - 1);
                model.follow = model.selection + 1 == len;
            }
        }
        KeyCode::Char('k') | KeyCode::Up => return move_up(model, 1),
        KeyCode::PageDown => {
            if len > 0 {
                model.selection = (model.selection + model.visible_height).min(len - 1);
                model.follow = model.selection + 1 == len;
            }
        }
        KeyCode::PageUp => return move_up(model, model.visible_height),
        KeyCode::Char('G') => {
            model.selection = len.saturating_sub(1);
            model.follow = true;
            model.footer = FooterState::Live;
        }
        KeyCode::Char('g') => {
            model.selection = 0;
            model.follow = false;
        }
        KeyCode::Char('t') => {
            model.reasoning_expanded = !model.reasoning_expanded;
            model.cache_dirty = true;
        }
        KeyCode::Enter => {
            if let Some(entry) = model.transcript.entries().get(model.selection)
                && entry_has_expandable_part(entry)
                && !model.expanded.remove(&entry.start_sequence)
            {
                model.expanded.insert(entry.start_sequence);
            }
            model.cache_dirty = true;
        }
        _ => return Vec::new(),
    }
    model.dirty = true;
    Vec::new()
}

fn move_up(model: &mut Model, amount: usize) -> Vec<Cmd> {
    if model.selection > 0 {
        model.selection = model.selection.saturating_sub(amount);
        model.follow = false;
        model.dirty = true;
        return Vec::new();
    }
    if model.transcript.has_older && !model.fetching {
        model.fetching = true;
        return vec![Cmd::FetchPage {
            before_sequence: model.transcript.next_before_sequence,
        }];
    }
    if !model.transcript.has_older {
        model.beginning = true;
        model.dirty = true;
    }
    Vec::new()
}

fn apply_page(
    model: &mut Model,
    result: Result<compozy_client::types::TranscriptPage, String>,
) -> Vec<Cmd> {
    model.fetching = false;
    let page = match result {
        Ok(page) => page,
        Err(error) => {
            model.warning = Some(error);
            model.dirty = true;
            return Vec::new();
        }
    };
    let was_empty = model.transcript.is_empty();
    let anchor = model
        .transcript
        .entries()
        .get(model.selection)
        .map(|entry| entry.start_sequence);
    model.transcript.prepend_page(page);
    if was_empty {
        model.selection = model.transcript.len().saturating_sub(1);
    } else if let Some(anchor) = anchor {
        model.selection = model
            .transcript
            .entries()
            .iter()
            .position(|entry| entry.start_sequence == anchor)
            .unwrap_or(model.selection);
    }
    model.beginning = !model.transcript.has_older;
    model.dirty = true;
    model.cache_dirty = true;
    if was_empty && !model.stopped {
        vec![Cmd::StartStream(model.transcript.cursor())]
    } else {
        Vec::new()
    }
}

fn apply_stream(model: &mut Model, event: TranscriptEvent) -> Vec<Cmd> {
    if model.stopped && !matches!(event, TranscriptEvent::Stopped(_)) {
        return Vec::new();
    }
    match event {
        TranscriptEvent::Snapshot(snapshot) => apply_snapshot(model, snapshot),
        TranscriptEvent::Delta(delta) => apply_delta(model, delta),
        TranscriptEvent::Stopped(stopped) => {
            model.stopped = true;
            model.footer = FooterState::Stopped {
                reason: stopped.stop_reason,
                detail: stopped.stop_detail,
            };
            if let Some(failure) = stopped.failure {
                model.warning = Some(format!("session failure: {failure}"));
            }
            model.dirty = true;
            vec![Cmd::StopStream]
        }
        TranscriptEvent::ServerError(error) => {
            model.warning = Some(error.error);
            model.dirty = true;
            Vec::new()
        }
        TranscriptEvent::Lost { attempt, .. } => {
            model.footer = if attempt >= 5 {
                FooterState::Offline
            } else {
                FooterState::Reconnecting(attempt)
            };
            model.dirty = true;
            Vec::new()
        }
        TranscriptEvent::Reconnected => {
            model.footer = FooterState::Live;
            model.dirty = true;
            Vec::new()
        }
        TranscriptEvent::Fatal(error) => {
            model.footer = FooterState::Fatal(error.to_string());
            model.dirty = true;
            vec![Cmd::StopStream]
        }
    }
}

fn apply_snapshot(model: &mut Model, snapshot: TranscriptSnapshot) -> Vec<Cmd> {
    let reset = snapshot.reset;
    let reason = snapshot.reason.clone().unwrap_or_else(|| "reset".into());
    let old_len = model.transcript.len();
    let old_cursor = model.transcript.cursor();
    model.transcript.apply_snapshot(snapshot);
    model.cache_dirty = true;
    if model.follow {
        model.selection = model.transcript.len().saturating_sub(1);
    } else {
        let new_count = model.transcript.len().saturating_sub(old_len);
        if new_count > 0 {
            model.footer = FooterState::NewBelow(new_count);
        }
    }
    model.dirty = true;
    if reset {
        model.footer = FooterState::Resynchronized(reason);
        let mut commands = Vec::new();
        if old_cursor != model.transcript.cursor() {
            commands.push(Cmd::StopStream);
            commands.push(Cmd::StartStream(model.transcript.cursor()));
        }
        commands.push(Cmd::ClearFooterAfter(Duration::from_secs(5)));
        commands
    } else {
        Vec::new()
    }
}

fn apply_delta(model: &mut Model, delta: TranscriptDelta) -> Vec<Cmd> {
    let additions = delta
        .entries
        .iter()
        .filter(|entry| model.transcript.entry(entry.start_sequence).is_none())
        .count();
    match model.transcript.apply_delta(delta.clone()) {
        Applied::Ok => {
            model.cache_dirty = true;
            if model.follow {
                model.selection = model.transcript.len().saturating_sub(1);
            } else if additions > 0 {
                model.footer = FooterState::NewBelow(additions);
            }
            model.dirty = true;
            Vec::new()
        }
        Applied::UnknownStart(_) => {
            if model.fetching {
                Vec::new()
            } else {
                model.fetching = true;
                vec![Cmd::FetchPage {
                    before_sequence: None,
                }]
            }
        }
        Applied::FenceMismatch => apply_snapshot(
            model,
            TranscriptSnapshot {
                epoch: delta.epoch,
                generation: delta.generation,
                entries: delta.entries,
                max_sequence: delta.max_sequence.max(delta.cursor),
                reset: true,
                reason: Some("epoch_mismatch".into()),
                ..TranscriptSnapshot::default()
            },
        ),
    }
}

fn entry_has_expandable_part(entry: &Entry) -> bool {
    entry
        .message
        .parts
        .iter()
        .any(|part| matches!(part, Part::Tool { .. } | Part::Permission { .. }))
}
