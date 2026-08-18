#![allow(dead_code)]
use batuta_tui::{
    Model, Msg, Request, RequestId,
    app::{AppMode, Settings},
    msg::ApiResponse,
    update, views,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

pub fn model() -> Model {
    let mut model = Model::new(Settings::default(), AppMode::Full);
    model.now_unix = 1_755_556_800;
    model
}

pub fn key(code: KeyCode) -> Msg {
    Msg::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

pub fn key_with(code: KeyCode, modifiers: KeyModifiers) -> Msg {
    Msg::Key(KeyEvent::new(code, modifiers))
}

pub fn respond(model: &mut Model, request: Request, response: ApiResponse) -> Vec<batuta_tui::Cmd> {
    let id = request.id();
    model
        .pending
        .insert(id, batuta_tui::app::PendingKind::Request(request));
    update(
        model,
        Msg::Api {
            request: id,
            result: Ok(response),
        },
    )
}

pub fn fail(model: &mut Model, request: Request, error: &str) -> Vec<batuta_tui::Cmd> {
    let id = request.id();
    model
        .pending
        .insert(id, batuta_tui::app::PendingKind::Request(request));
    update(
        model,
        Msg::Api {
            request: id,
            result: Err(error.into()),
        },
    )
}

pub fn id(value: u64) -> RequestId {
    RequestId(value)
}

pub fn render(model: &Model, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| views::view(model, frame)).unwrap();
    terminal.backend().to_string()
}

pub fn session_page() -> compozy_client::types::SessionPage {
    serde_json::from_value(serde_json::json!({
        "sessions": [
            {"id":"sess-a","agent_name":"batuta","name":"spike plan","state":"active","badge":"running","activity":{"last_activity_at":"2025-08-18T23:59:48Z"}},
            {"id":"sess-b","agent_name":"general","name":null,"state":"stopped","updated_at":"2025-08-17T00:00:00Z"}
        ],
        "page":{"has_more":true,"limit":50}
    })).unwrap()
}

pub fn runs_page(live: bool) -> compozy_client::types::LoopRunPage {
    let status = if live { "running" } else { "done" };
    serde_json::from_value(serde_json::json!({
        "runs": [
            {"id":"looprun-parent1234","workspace_id":"ws-test","loop_name":"batuta-deliver","status":status,"generation":1,"created_at":"2025-08-18T23:50:00Z","started_at":"2025-08-18T23:50:01Z","last_progress_at":"2025-08-18T23:58:00Z"},
            {"id":"looprun-child5678","workspace_id":"ws-test","loop_name":"implement-tasks","status":"done","generation":1,"created_at":"2025-08-18T22:00:00Z","started_at":"2025-08-18T22:00:01Z","last_progress_at":"2025-08-18T23:40:00Z","parent_loop_run_id":"looprun-parent1234"}
        ],
        "aggregates":{"total":2,"live":if live {1} else {0},"terminal":if live {1} else {2},"succeeded":1,"failed":0}
    })).unwrap()
}

pub fn sessions_request(id: u64) -> Request {
    Request::Sessions {
        id: RequestId(id),
        workspace: "ws-test".into(),
        agent: Some("batuta".into()),
        limit: 50,
    }
}

pub fn runs_request(id: u64) -> Request {
    Request::Runs {
        id: RequestId(id),
        workspace: "ws-test".into(),
        loop_name: Some("batuta-deliver".into()),
        limit: 50,
    }
}
