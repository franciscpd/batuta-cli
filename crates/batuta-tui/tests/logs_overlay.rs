#[path = "support/panels.rs"]
mod panels_support;

use batuta_tui::{
    AnyStreamEvent, ApiResponse, Cmd, Msg, Request, StreamId,
    app::{Detail, Panel, SessionDetail},
    cmd::LogScope,
    update,
};
use compozy_client::{
    Error, StreamEvent,
    types::{LogEvent, Session},
};
use crossterm::event::KeyCode;
use panels_support::{key, model, render, respond};

fn log(id: &str, outcome: &str, summary: &str) -> LogEvent {
    serde_json::from_value(serde_json::json!({
        "id":id, "workspace_id":"ws-test", "session_id":"sess-log", "type":"tool",
        "agent_name":"batuta", "outcome":outcome, "summary":summary,
        "timestamp":"2026-08-18T20:05:52Z"
    }))
    .unwrap()
}

fn session_model() -> batuta_tui::Model {
    let mut model = model();
    model.focus = Panel::Detail;
    model.detail = Detail::Session(Box::new(SessionDetail::new(Session {
        id: "sess-log".into(),
        agent_name: "batuta".into(),
        state: "active".into(),
        ..Session::default()
    })));
    model
}

fn open(model: &mut batuta_tui::Model) -> (Request, Vec<Cmd>) {
    let commands = update(model, key(KeyCode::Char('L')));
    let request = commands
        .iter()
        .find_map(|cmd| match cmd {
            Cmd::Get(request @ Request::Logs { .. }) => Some(request.clone()),
            _ => None,
        })
        .expect("logs get");
    (request, commands)
}

#[test]
fn ut_610_session_focus_opens_fetches_streams_and_snapshots() {
    let mut model = session_model();
    let (request, commands) = open(&mut model);
    assert!(
        matches!(request, Request::Logs { scope:LogScope::Session(ref id), limit:200, .. } if id == "sess-log")
    );
    assert!(
        commands.contains(&Cmd::StartStream(StreamId::Logs(LogScope::Session(
            "sess-log".into()
        ))))
    );
    respond(
        &mut model,
        request,
        ApiResponse::Logs(vec![log("1", "ok", "started")]),
    );
    let screen = render(&model, 120, 40);
    assert!(
        screen.contains("logs · session sess-log · live"),
        "{screen}"
    );
    insta::assert_snapshot!("logs_session_120x40", screen);
    insta::assert_snapshot!("logs_session_100x30", render(&model, 100, 30));
}

#[test]
fn ut_611_run_focus_uses_run_query_scope() {
    let mut model = model();
    model.focus = Panel::Detail;
    model.detail = Detail::Run(Box::new(batuta_tui::app::RunDetail {
        run: None,
        run_id: "looprun-log".into(),
        events: vec![],
        selection: 0,
        confirm: None,
        stream: Default::default(),
    }));
    let (request, _) = open(&mut model);
    assert!(
        matches!(request, Request::Logs { scope:LogScope::Run(ref id), .. } if id == "looprun-log")
    );
}

#[test]
fn ut_612_error_toggle_refetches_and_restarts_stream() {
    let mut model = session_model();
    open(&mut model);
    let commands = update(&mut model, key(KeyCode::Char('e')));
    assert!(commands.iter().any(|cmd| matches!(
        cmd,
        Cmd::Get(Request::Logs {
            error_only: true,
            ..
        })
    )));
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::StopStream(StreamId::Logs(_))))
    );
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::StartStream(StreamId::Logs(_))))
    );
    assert!(render(&model, 120, 40).contains("· errors"));
}

#[test]
fn ut_613_escape_and_l_close_and_stop_logs_stream() {
    for close in [KeyCode::Esc, KeyCode::Char('L')] {
        let mut model = session_model();
        open(&mut model);
        let commands = update(&mut model, key(close));
        assert!(model.overlay.is_none());
        assert!(
            commands
                .iter()
                .any(|cmd| matches!(cmd, Cmd::StopStream(StreamId::Logs(_))))
        );
    }
}

#[test]
fn ut_614_log_rows_and_errors_are_rendered() {
    let mut model = session_model();
    let (request, _) = open(&mut model);
    respond(
        &mut model,
        request,
        ApiResponse::Logs(vec![log("1", "ok", "all good"), log("2", "error", "boom")]),
    );
    let screen = render(&model, 100, 30);
    for expected in ["20:05:52", "tool", "○", "!", "batuta", "all good", "boom"] {
        assert!(screen.contains(expected), "missing {expected:?}\n{screen}");
    }
    insta::assert_snapshot!("logs_errors_100x30", screen);
    insta::assert_snapshot!("logs_errors_120x40", render(&model, 120, 40));
}

#[test]
fn ut_615_no_focus_uses_workspace_scope() {
    let mut model = model();
    model.focus = Panel::Sessions;
    model.sessions.items.clear();
    let (request, _) = open(&mut model);
    assert!(matches!(
        request,
        Request::Logs {
            scope: LogScope::Workspace,
            ..
        }
    ));
}

#[test]
fn ut_616_bad_cursor_refetches_latest_and_marks_reset() {
    let mut model = session_model();
    open(&mut model);
    let scope = LogScope::Session("sess-log".into());
    let commands = update(
        &mut model,
        Msg::Stream {
            id: StreamId::Logs(scope),
            event: AnyStreamEvent::Logs(StreamEvent::Fatal(Error::Daemon {
                status: 400,
                message: "bad cursor".into(),
                code: Some("bad_cursor".into()),
                details: None,
                diagnostic: None,
            })),
        },
    );
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Get(Request::Logs { limit: 200, .. })))
    );
    assert!(render(&model, 120, 40).contains("cursor reset"));
}

#[test]
fn logs_overlay_shrink_closes_and_stops_its_stream() {
    let mut model = session_model();
    open(&mut model);
    let stream = StreamId::Logs(LogScope::Session("sess-log".into()));
    let commands = update(&mut model, Msg::Resize(72, 20));
    assert!(model.overlay.is_none());
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::StopStream(StreamId::Logs(_))))
    );
    assert!(!model.active_streams.contains(&stream));
}
