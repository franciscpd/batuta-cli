use crate::{
    app::composer::ComposerState,
    cmd::{Cmd, LogScope, Request, RequestId, StreamId, TimerId},
    theme::{Theme, ThemeVariant},
    transcript::TranscriptState,
};
use compozy_client::types::{
    ApproveRequest, Clarification, Decision, Entry, LogEvent, LoopEvent, LoopRunAggregates,
    LoopRunDetail, PermissionData, Session, Timestamp, TranscriptPage,
};
use ratatui::text::Text;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppMode {
    #[default]
    Full,
    TailOnly,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Panel {
    #[default]
    Sessions,
    Runs,
    Attention,
    Detail,
}

impl Panel {
    pub const ALL: [Self; 4] = [Self::Sessions, Self::Runs, Self::Attention, Self::Detail];
    pub const fn next(self) -> Self {
        Self::ALL[(self as usize + 1) % 4]
    }
    pub const fn previous(self) -> Self {
        Self::ALL[(self as usize + 3) % 4]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorMode {
    #[default]
    Auto,
    Never,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeMode {
    #[default]
    Auto,
    Dark,
    Light,
}

impl From<ThemeMode> for ThemeVariant {
    fn from(value: ThemeMode) -> Self {
        match value {
            ThemeMode::Auto => Self::Auto,
            ThemeMode::Dark => Self::Dark,
            ThemeMode::Light => Self::Light,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preset {
    pub agent: String,
    pub loop_name: String,
    pub provider: String,
    pub model: String,
}
impl Default for Preset {
    fn default() -> Self {
        Self {
            agent: "batuta".into(),
            loop_name: "batuta-deliver".into(),
            provider: "claude".into(),
            model: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiSettings {
    pub color: ColorMode,
    pub theme: ThemeMode,
    pub fps: u16,
    pub sessions_limit: u64,
    pub runs_limit: u64,
    pub notify: bool,
}
impl Default for UiSettings {
    fn default() -> Self {
        Self {
            color: ColorMode::Auto,
            theme: ThemeMode::Auto,
            fps: 30,
            sessions_limit: 50,
            runs_limit: 50,
            notify: true,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceRef {
    pub id: String,
    pub name: String,
    pub root_dir: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceCandidate {
    pub name: String,
    pub root_dir: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub preset: Preset,
    pub ui: UiSettings,
    pub workspace: Option<WorkspaceRef>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            preset: Preset::default(),
            ui: UiSettings::default(),
            workspace: Some(WorkspaceRef {
                id: "ws-test".into(),
                name: "workspace".into(),
                root_dir: String::new(),
            }),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ListState<T> {
    pub items: Vec<T>,
    pub selected: Option<usize>,
    pub filter: String,
    pub filter_focused: bool,
}
impl<T> ListState<T> {
    pub fn set_items(&mut self, items: Vec<T>) {
        let old = self.selected;
        self.items = items;
        self.selected = if self.items.is_empty() {
            None
        } else {
            Some(old.unwrap_or(0).min(self.items.len() - 1))
        };
    }
    pub fn select_next(&mut self) {
        if !self.items.is_empty() {
            self.selected = Some((self.selected.unwrap_or(0) + 1).min(self.items.len() - 1));
        }
    }
    pub fn select_previous(&mut self) {
        if !self.items.is_empty() {
            self.selected = Some(self.selected.unwrap_or(0).saturating_sub(1));
        }
    }
    pub fn selected(&self) -> Option<&T> {
        self.selected.and_then(|index| self.items.get(index))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionRow {
    pub id: String,
    pub agent: String,
    pub name: Option<String>,
    pub state: String,
    pub badge: Option<String>,
    pub last_activity_at: Option<Timestamp>,
}
impl From<&Session> for SessionRow {
    fn from(value: &Session) -> Self {
        Self {
            id: value.id.clone(),
            agent: value.agent_name.clone(),
            name: value.name.clone(),
            state: value.state.clone(),
            badge: value.badge.clone(),
            last_activity_at: value
                .activity
                .as_ref()
                .and_then(|activity| activity.last_activity_at.clone())
                .or_else(|| value.updated_at.clone()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunRow {
    pub id: String,
    pub loop_name: String,
    pub status: String,
    pub parent_id: String,
    pub last_progress_at: Option<Timestamp>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttentionSource {
    Permission {
        session_id: String,
        request_id: String,
        turn_id: String,
        allow_always: bool,
    },
    Clarification {
        session_id: String,
        request_id: String,
        choices: Vec<String>,
        deadline: Option<Timestamp>,
    },
    Overview {
        kind: String,
        task_id: String,
        run_id: Option<String>,
        session_id: Option<String>,
        actions: Vec<String>,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AttentionItem {
    pub source: Option<AttentionSource>,
    pub title: String,
    pub detail: String,
    pub session_id: Option<String>,
    pub run_id: Option<String>,
    pub at: Option<Timestamp>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FooterState {
    #[default]
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
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum StreamStatus {
    #[default]
    Live,
    Stale,
    Reconnecting(u32),
    Stopped,
    Fatal(String),
}
#[derive(Clone, Debug, Default)]
pub struct TranscriptView {
    pub selection: usize,
    pub selected_source_start_sequence: Option<i64>,
    pub reasoning_expanded: bool,
    pub raw_debug: bool,
    pub expanded: BTreeSet<i64>,
    pub follow: bool,
    pub footer: FooterState,
    pub visible_height: usize,
    pub warning: Option<String>,
    pub fetching: bool,
    pub stopped: bool,
    pub beginning: bool,
    pub render_cache: HashMap<RenderCacheKey, Text<'static>>,
    pub cache_dirty: bool,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct RenderCacheKey {
    pub start_sequence: i64,
    pub sequence: i64,
    pub width: u16,
    pub reasoning_expanded: bool,
    pub raw_debug: bool,
    pub expanded: bool,
    pub color: bool,
    pub theme: ThemeVariant,
}

impl RenderCacheKey {
    pub fn for_entry(
        entry: &compozy_client::types::Entry,
        width: u16,
        reasoning_expanded: bool,
        raw_debug: bool,
        expanded: bool,
        color: bool,
        theme: ThemeVariant,
    ) -> Self {
        Self {
            start_sequence: entry.start_sequence,
            sequence: entry.sequence,
            width,
            reasoning_expanded,
            raw_debug,
            expanded,
            color,
            theme,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SessionDetail {
    pub session: Session,
    pub transcript: TranscriptState,
    pub view: TranscriptView,
    pub composer: ComposerState,
    pub chooser: Option<ModeChooser>,
    pub confirm: Option<Confirm>,
    pub stream: StreamStatus,
}
impl SessionDetail {
    pub fn new(session: Session) -> Self {
        let stopped = session.state == "stopped";
        Self {
            session,
            transcript: TranscriptState::default(),
            view: TranscriptView {
                selected_source_start_sequence: None,
                follow: true,
                fetching: true,
                stopped,
                footer: if stopped {
                    FooterState::Stopped {
                        reason: None,
                        detail: None,
                    }
                } else {
                    FooterState::Live
                },
                visible_height: 20,
                ..TranscriptView::default()
            },
            composer: ComposerState::default(),
            chooser: None,
            confirm: None,
            stream: if stopped {
                StreamStatus::Stopped
            } else {
                StreamStatus::Live
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct RunDetail {
    pub run: Option<LoopRunDetail>,
    pub run_id: String,
    pub events: Vec<LoopEvent>,
    pub selection: usize,
    pub confirm: Option<Confirm>,
    pub stream: StreamStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModeChooser {
    pub selected: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Confirm {
    pub prompt: String,
}

#[derive(Clone, Debug, Default)]
pub enum Detail {
    #[default]
    Empty,
    Session(Box<SessionDetail>),
    Run(Box<RunDetail>),
}
#[derive(Clone, Debug, PartialEq)]
pub enum Overlay {
    Help {
        scroll: usize,
    },
    Logs {
        scope: LogScope,
        error_only: bool,
        events: Vec<LogEvent>,
        selection: usize,
        cursor_reset: bool,
    },
    WorkspacePicker {
        selected: Option<usize>,
        items: Vec<WorkspaceRef>,
        at_start: bool,
    },
    WorkspaceOnboarding {
        candidate: WorkspaceCandidate,
        confirming: bool,
        adding: bool,
        booting: bool,
        refresh_required: bool,
        registration_complete: bool,
        registration_unsupported: bool,
        message: Option<String>,
        technical_details: Option<String>,
        details_expanded: bool,
    },
    Clarify {
        session_id: String,
        request_id: String,
        question: String,
        deadline: Option<Timestamp>,
        text: String,
        choices: Vec<String>,
        selected: usize,
        text_focused: bool,
        hint: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Error,
    Info,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toast {
    pub kind: ToastKind,
    pub text: String,
    pub sticky: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DaemonStatus {
    pub status: String,
    pub version: Option<String>,
    pub poll_ok: bool,
}

/// Single derivation point for the daemon's UI-facing state (ADR-002).
/// Every consumer reads this instead of comparing `DaemonStatus.status`
/// or the offline predicate independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DaemonState {
    Connected,
    Draining,
    Offline,
}

impl DaemonState {
    pub fn derive(status: &DaemonStatus, offline: bool) -> Self {
        if status.status == "draining" {
            DaemonState::Draining
        } else if offline {
            DaemonState::Offline
        } else {
            DaemonState::Connected
        }
    }
}

#[derive(Clone, Debug)]
pub enum PendingKind {
    Request(Request),
}

#[derive(Clone, Debug)]
pub struct Model {
    pub mode: AppMode,
    pub settings: Settings,
    pub workspace: Option<WorkspaceRef>,
    pub startup_candidate: Option<WorkspaceCandidate>,
    pub startup_boot_pending: BTreeSet<RequestId>,
    pub focus: Panel,
    pub sessions: ListState<SessionRow>,
    pub runs: ListState<RunRow>,
    pub attention: Vec<AttentionItem>,
    pub attention_selected: Option<usize>,
    pub attention_permissions: HashMap<String, Vec<PermissionData>>,
    pub attention_clarifications: HashMap<String, Vec<Clarification>>,
    pub attention_overview: Vec<compozy_client::types::AttentionItem>,
    pub attention_overview_total: u64,
    pub attention_overview_unavailable: bool,
    pub attention_confirm: Option<(usize, Decision)>,
    pub inline_permission_confirm: Option<(String, ApproveRequest)>,
    pub sessions_unfiltered: Vec<SessionRow>,
    pub sessions_all_agents: bool,
    pub sessions_has_more: bool,
    pub catalog_debounce_armed: bool,
    pub create_session_pending: bool,
    pub prompt_pending: bool,
    pub failed_prompt: Option<(String, compozy_client::types::PromptRequest)>,
    pub app_created_sessions: HashSet<String>,
    pub runs_unfiltered: Vec<RunRow>,
    pub runs_all_loops: bool,
    pub runs_aggregates: Option<LoopRunAggregates>,
    pub runs_stale: bool,
    pub detail: Detail,
    pub overlay: Option<Overlay>,
    pub toast: Option<Toast>,
    pub daemon: DaemonStatus,
    pub size: (u16, u16),
    pub now_unix: i64,
    pub dirty: bool,
    pub pending: BTreeMap<RequestId, PendingKind>,
    pub late_writes: BTreeMap<RequestId, Request>,
    pub active_streams: HashSet<StreamId>,
    pub stream_status: HashMap<StreamId, StreamStatus>,
    pub stream_cursors: HashMap<StreamId, String>,
    pub theme: Theme,
    pub quit_guard: bool,
    pub terminal_focused: bool,
    next_request: u64,
    next_message: u64,
}

impl Model {
    pub fn new(settings: Settings, mode: AppMode) -> Self {
        let workspace = settings.workspace.clone();
        let color = settings.ui.color != ColorMode::Never;
        let theme_mode = settings.ui.theme;
        Self {
            mode,
            settings,
            workspace,
            startup_candidate: None,
            startup_boot_pending: BTreeSet::new(),
            focus: if mode == AppMode::TailOnly {
                Panel::Detail
            } else {
                Panel::Sessions
            },
            sessions: ListState::default(),
            runs: ListState::default(),
            attention: Vec::new(),
            attention_selected: None,
            attention_permissions: HashMap::new(),
            attention_clarifications: HashMap::new(),
            attention_overview: Vec::new(),
            attention_overview_total: 0,
            attention_overview_unavailable: false,
            attention_confirm: None,
            inline_permission_confirm: None,
            sessions_unfiltered: Vec::new(),
            sessions_all_agents: false,
            sessions_has_more: false,
            catalog_debounce_armed: false,
            create_session_pending: false,
            prompt_pending: false,
            failed_prompt: None,
            app_created_sessions: HashSet::new(),
            runs_unfiltered: Vec::new(),
            runs_all_loops: false,
            runs_aggregates: None,
            runs_stale: false,
            detail: Detail::Empty,
            overlay: None,
            toast: None,
            daemon: DaemonStatus {
                status: "running".into(),
                version: None,
                poll_ok: true,
            },
            size: (100, 30),
            now_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |value| value.as_secs() as i64),
            dirty: true,
            pending: BTreeMap::new(),
            late_writes: BTreeMap::new(),
            active_streams: HashSet::new(),
            stream_status: HashMap::new(),
            stream_cursors: HashMap::new(),
            theme: Theme::with_variant(
                color,
                theme_mode.into(),
                std::env::var("COLORFGBG").ok().as_deref(),
            ),
            quit_guard: false,
            terminal_focused: true,
            next_request: 1,
            next_message: 1,
        }
    }
    pub fn tail(header: SessionHeader) -> Self {
        let settings = Settings {
            workspace: Some(WorkspaceRef {
                id: header.workspace_id.clone(),
                name: header.workspace.clone(),
                root_dir: String::new(),
            }),
            ..Settings::default()
        };
        let mut model = Self::new(settings, AppMode::TailOnly);
        let session = Session {
            id: header.session_id,
            agent_name: header.agent,
            name: header.name,
            state: header.state,
            stop_reason: None,
            stop_detail: None,
            ..Session::default()
        };
        let mut detail = SessionDetail::new(session);
        detail.view.warning = header.warning;
        model.detail = Detail::Session(Box::new(detail));
        model
    }
    pub fn start_workspace_onboarding(&mut self, candidate: WorkspaceCandidate) {
        self.startup_candidate = Some(candidate.clone());
        self.overlay = Some(Overlay::WorkspaceOnboarding {
            candidate,
            confirming: false,
            adding: false,
            booting: false,
            refresh_required: false,
            registration_complete: false,
            registration_unsupported: false,
            message: None,
            technical_details: None,
            details_expanded: false,
        });
        self.dirty = true;
    }
    pub fn allocate(&mut self, make: impl FnOnce(RequestId) -> Request) -> Request {
        let id = RequestId(self.next_request);
        self.next_request += 1;
        let request = make(id);
        self.pending
            .insert(id, PendingKind::Request(request.clone()));
        request
    }
    pub fn complete_workspace_boot(&mut self, id: RequestId) {
        if self.startup_boot_pending.remove(&id) && self.startup_boot_pending.is_empty() {
            self.startup_candidate = None;
            self.overlay = None;
            self.dirty = true;
        }
    }
    pub fn fail_workspace_boot(&mut self, id: RequestId, error: String) -> Option<Vec<Cmd>> {
        if !self.startup_boot_pending.remove(&id) {
            return None;
        }
        for id in std::mem::take(&mut self.startup_boot_pending) {
            self.pending.remove(&id);
        }
        let candidate = self.startup_candidate.clone()?;
        self.overlay = Some(Overlay::WorkspaceOnboarding {
            candidate,
            confirming: false,
            adding: false,
            booting: false,
            refresh_required: false,
            registration_complete: true,
            registration_unsupported: false,
            message: Some(format!("workspace selected; startup failed: {error}")),
            technical_details: None,
            details_expanded: false,
        });
        let commands = self.all_stop_cmds();
        self.active_streams.clear();
        self.dirty = true;
        Some(commands)
    }
    pub fn message_ids(&mut self) -> (String, String) {
        let value = self.next_message;
        self.next_message += 1;
        (format!("msg_{value:016x}"), format!("idem_{value:016x}"))
    }
    pub fn initial_cmds(&mut self) -> Vec<Cmd> {
        if self.mode == AppMode::TailOnly {
            if let (Some(ws), Detail::Session(detail)) = (self.workspace.clone(), &self.detail) {
                let session = detail.session.id.clone();
                return vec![
                    Cmd::Get(self.allocate(|id| Request::TranscriptPage {
                        id,
                        workspace: ws.id,
                        session,
                        before_sequence: None,
                    })),
                    Cmd::Render,
                ];
            }
            return vec![Cmd::Render];
        }
        if self.startup_candidate.is_some() {
            return vec![Cmd::Render];
        }
        let Some(workspace) = self.workspace.clone() else {
            self.overlay = Some(Overlay::WorkspacePicker {
                selected: None,
                items: Vec::new(),
                at_start: true,
            });
            let request = self.allocate(|id| Request::Workspaces { id });
            return vec![Cmd::Get(request), Cmd::Render];
        };
        let agent = self.settings.preset.agent.clone();
        let sessions_limit = self.settings.ui.sessions_limit;
        let loop_name = self.settings.preset.loop_name.clone();
        let runs_limit = self.settings.ui.runs_limit;
        let sessions = self.allocate(|id| Request::Sessions {
            id,
            workspace: workspace.id.clone(),
            agent: Some(agent),
            limit: sessions_limit,
        });
        let runs = self.allocate(|id| Request::Runs {
            id,
            workspace: workspace.id.clone(),
            loop_name: Some(loop_name),
            limit: runs_limit,
        });
        let overview = self.allocate(|id| Request::Overview {
            id,
            workspace: workspace.id,
        });
        self.active_streams.insert(StreamId::Catalog);
        vec![
            Cmd::Get(sessions),
            Cmd::Get(runs),
            Cmd::Get(overview),
            Cmd::StartStream(StreamId::Catalog),
            Cmd::After(Duration::from_secs(30), TimerId::StatusPoll),
            Cmd::After(Duration::from_secs(30), TimerId::AttentionPoll),
            Cmd::Render,
        ]
    }
    pub fn session_detail(&self) -> Option<&SessionDetail> {
        match &self.detail {
            Detail::Session(value) => Some(value),
            _ => None,
        }
    }
    pub fn session_detail_mut(&mut self) -> Option<&mut SessionDetail> {
        match &mut self.detail {
            Detail::Session(value) => Some(value),
            _ => None,
        }
    }
    pub fn composer_focused(&self) -> bool {
        self.session_detail()
            .is_some_and(|detail| detail.composer.focused)
    }
    pub fn text_field_focused(&self) -> bool {
        self.composer_focused()
            || self.sessions.filter_focused
            || self.runs.filter_focused
            || matches!(
                &self.overlay,
                Some(Overlay::Clarify {
                    text_focused: true,
                    ..
                })
            )
    }
    pub fn draft_nonempty(&self) -> bool {
        self.session_detail()
            .is_some_and(|detail| !detail.composer.is_empty())
    }
    pub fn too_small(&self) -> bool {
        self.size.0 < 80 || self.size.1 < 24
    }
    pub fn set_success_toast(&mut self, text: impl Into<String>) -> Vec<Cmd> {
        self.toast = Some(Toast {
            kind: ToastKind::Success,
            text: text.into(),
            sticky: false,
        });
        self.dirty = true;
        vec![Cmd::After(Duration::from_secs(3), TimerId::ToastExpiry)]
    }
    pub fn set_sticky_toast(&mut self, text: impl Into<String>) {
        self.toast = Some(Toast {
            kind: ToastKind::Error,
            text: text.into(),
            sticky: true,
        });
        self.dirty = true;
    }
    pub fn all_stop_cmds(&self) -> Vec<Cmd> {
        let mut ids = self.active_streams.iter().cloned().collect::<Vec<_>>();
        ids.sort_by_key(ToString::to_string);
        ids.into_iter().map(Cmd::StopStream).collect()
    }
    pub fn is_offline(&self) -> bool {
        !self.daemon.poll_ok
            && !self.active_streams.is_empty()
            && self.active_streams.iter().all(|id| {
                matches!(
                    self.stream_status.get(id),
                    Some(StreamStatus::Stale | StreamStatus::Fatal(_))
                )
            })
    }
    pub fn daemon_state(&self) -> DaemonState {
        DaemonState::derive(&self.daemon, self.is_offline())
    }
    /// Refuses a write action while the daemon is draining, surfacing the
    /// `_dx.md` Errors-table message. Returns `true` when the caller must
    /// abort the write.
    pub fn refuse_write(&mut self, verb: &str) -> bool {
        if self.daemon_state() == DaemonState::Draining {
            self.set_sticky_toast(format!(
                "can't {verb} — daemon draining, try again once it recovers"
            ));
            true
        } else {
            false
        }
    }
}

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

pub fn entry_has_expandable_part(entry: &Entry) -> bool {
    entry.message.parts.iter().any(|part| {
        matches!(
            part,
            compozy_client::types::Part::Tool { .. }
                | compozy_client::types::Part::Permission { .. }
        )
    })
}
pub fn page_into_detail(detail: &mut SessionDetail, page: TranscriptPage) {
    let was_empty = detail.transcript.is_empty();
    let has_older = page.has_older;
    detail.transcript.prepend_page(page);
    if was_empty {
        detail.view.selection = detail
            .transcript
            .presentation_rows(detail.view.raw_debug)
            .len()
            .saturating_sub(1);
    }
    detail.view.fetching = false;
    detail.view.beginning = !has_older;
    detail.view.cache_dirty = true;
}

#[cfg(test)]
mod tests {
    use super::{SessionDetail, page_into_detail};
    use compozy_client::types::{Session, TranscriptPage};

    #[test]
    fn initial_page_selection_uses_grouped_presentation_rows() {
        let mut detail = SessionDetail::new(Session::default());
        let page: TranscriptPage = serde_json::from_value(serde_json::json!({
            "entries": (1..=6).map(|sequence| serde_json::json!({
                "message": {
                    "id": format!("message-{sequence}"),
                    "role": "assistant",
                    "parts": [{
                        "type": "tool-test",
                        "state": "completed",
                        "name": "test"
                    }]
                },
                "start_sequence": sequence,
                "sequence": sequence
            })).collect::<Vec<_>>(),
            "epoch": 1,
            "generation": 1,
            "max_sequence": 6
        }))
        .unwrap();

        page_into_detail(&mut detail, page);

        assert_eq!(detail.transcript.presentation_rows(false).len(), 1);
        assert_eq!(detail.view.selection, 0);
    }
}
