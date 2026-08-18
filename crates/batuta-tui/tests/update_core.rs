use batuta_tui::{
    app::{
        AppMode, ColorMode, Detail, Model, Overlay, Panel, SessionHeader, SessionRow, Settings,
        StreamStatus, WorkspaceRef,
    },
    cmd::{Cmd, Request, RequestId, StreamId, TimerId},
    msg::{AnyStreamEvent, ApiResponse, Msg},
    update,
    views::header,
};
use compozy_client::StreamEvent;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::time::Duration;

fn press(code: KeyCode) -> Msg {
    Msg::Key(KeyEvent::new(code, KeyModifiers::NONE))
}
fn full() -> Model {
    Model::new(Settings::default(), AppMode::Full)
}

#[test]
fn ut_400_initial_commands() {
    let mut model = full();
    let commands = model.initial_cmds();
    assert_eq!(model.focus, Panel::Sessions);
    assert!(matches!(model.detail, Detail::Empty));
    assert_eq!(commands.len(), 7);
    assert!(matches!(commands[0], Cmd::Get(Request::Sessions { .. })));
    assert!(matches!(commands[1], Cmd::Get(Request::Runs { .. })));
    assert!(matches!(commands[2], Cmd::Get(Request::Overview { .. })));
    assert_eq!(commands[3], Cmd::StartStream(StreamId::Catalog));
    assert_eq!(
        commands[4],
        Cmd::After(Duration::from_secs(30), TimerId::StatusPoll)
    );
    assert_eq!(
        commands[5],
        Cmd::After(Duration::from_secs(30), TimerId::AttentionPoll)
    );
    assert_eq!(commands[6], Cmd::Render);
}
#[test]
fn ut_401_picker_fallback() {
    let settings = Settings {
        workspace: None,
        ..Settings::default()
    };
    let mut model = Model::new(settings, AppMode::Full);
    let commands = model.initial_cmds();
    assert!(matches!(
        model.overlay,
        Some(Overlay::WorkspacePicker { at_start: true, .. })
    ));
    assert!(matches!(
        commands.first(),
        Some(Cmd::Get(Request::Workspaces { .. }))
    ));
}
#[test]
fn ut_402_too_small_ignores_keys_and_restores_state() {
    let mut model = full();
    update(&mut model, Msg::Resize(72, 20));
    let focus = model.focus;
    assert!(update(&mut model, press(KeyCode::Tab)).is_empty());
    assert_eq!(model.focus, focus);
    update(&mut model, Msg::Resize(100, 30));
    assert_eq!(model.focus, focus);
}
#[test]
fn ut_403_no_color() {
    let mut settings = Settings::default();
    settings.ui.color = ColorMode::Never;
    assert!(!Model::new(settings, AppMode::Full).theme.color);
}
#[test]
fn ut_410_focus_router() {
    let mut model = full();
    for expected in [
        Panel::Runs,
        Panel::Attention,
        Panel::Detail,
        Panel::Sessions,
    ] {
        update(&mut model, press(KeyCode::Tab));
        assert_eq!(model.focus, expected);
    }
    update(
        &mut model,
        Msg::Key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
    );
    assert_eq!(model.focus, Panel::Detail);
    update(&mut model, press(KeyCode::Char('2')));
    assert_eq!(model.focus, Panel::Runs);
}
#[test]
fn ut_411_and_ut_412_selection_opens_and_follows() {
    let mut model = full();
    model.sessions.set_items(vec![
        SessionRow {
            id: "sess-a".into(),
            agent: "batuta".into(),
            name: None,
            state: "active".into(),
            ..SessionRow::default()
        },
        SessionRow {
            id: "sess-b".into(),
            agent: "batuta".into(),
            name: None,
            state: "active".into(),
            ..SessionRow::default()
        },
    ]);
    update(&mut model, press(KeyCode::Char('j')));
    let commands = update(&mut model, press(KeyCode::Enter));
    assert!(matches!(model.detail,Detail::Session(ref detail)if detail.session.id=="sess-b"));
    assert!(commands.iter().any(
        |command| matches!(command,Cmd::StartStream(StreamId::Transcript(id))if id=="sess-b")
    ));
    model.focus = Panel::Sessions;
    model.last_list_focus = Panel::Sessions;
    let commands = update(&mut model, Msg::Timer(TimerId::DetailSwitchDebounce));
    assert!(commands.iter().any(
        |command| matches!(command,Cmd::Get(Request::Session{session,..})if session=="sess-b")
    ));
}
#[test]
fn ut_413_and_ut_414_escape_and_digits_in_text() {
    let mut model = full();
    model.sessions.filter_focused = true;
    update(&mut model, press(KeyCode::Char('4')));
    assert_eq!(model.sessions.filter, "4");
    assert_eq!(model.focus, Panel::Sessions);
    update(&mut model, press(KeyCode::Esc));
    assert!(!model.sessions.filter_focused);
    assert!(model.sessions.filter.is_empty());
    model.overlay = Some(Overlay::Help { scroll: 0 });
    update(&mut model, press(KeyCode::Esc));
    assert!(model.overlay.is_none());
}
#[test]
fn ut_415_and_ut_416_empty_and_clamp() {
    let mut model = full();
    assert!(model.sessions.selected.is_none());
    assert!(update(&mut model, press(KeyCode::Enter)).is_empty());
    model
        .sessions
        .set_items(vec![Default::default(), Default::default()]);
    model.sessions.selected = Some(1);
    model.sessions.set_items(vec![Default::default()]);
    assert_eq!(model.sessions.selected, Some(0));
}
#[test]
fn ut_417_mouse_ignored() {
    let mut model = full();
    model.dirty = false;
    let message = Msg::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    });
    assert!(update(&mut model, message).is_empty());
    assert!(!model.dirty);
}
#[test]
fn ut_440_443_help_context() {
    let mut model = full();
    update(&mut model, press(KeyCode::Char('?')));
    assert!(matches!(model.overlay, Some(Overlay::Help { .. })));
    update(&mut model, press(KeyCode::Char('?')));
    assert!(model.overlay.is_none());
}
#[test]
fn ut_443_question_types_and_f1_opens_help() {
    let mut model = Model::tail(SessionHeader {
        workspace: "w".into(),
        workspace_id: "ws".into(),
        session_id: "s".into(),
        agent: "batuta".into(),
        name: None,
        state: "active".into(),
        warning: None,
    });
    model.session_detail_mut().unwrap().composer.focused = true;
    update(&mut model, press(KeyCode::Char('?')));
    assert_eq!(model.session_detail().unwrap().composer.text(), "?");
    update(&mut model, press(KeyCode::F(1)));
    assert!(matches!(model.overlay, Some(Overlay::Help { .. })));
}
#[test]
fn ut_450_and_ut_452_quit_has_no_posts() {
    let mut model = full();
    model.active_streams.insert(StreamId::Catalog);
    let commands = update(&mut model, press(KeyCode::Char('q')));
    assert!(commands.contains(&Cmd::StopStream(StreamId::Catalog)));
    assert!(matches!(commands.last(), Some(Cmd::Quit)));
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Cmd::Post(_)))
    );
}
#[test]
fn ut_451_quit_guard_and_expiry() {
    let mut model = Model::tail(SessionHeader {
        workspace: "w".into(),
        workspace_id: "ws".into(),
        session_id: "s".into(),
        agent: "batuta".into(),
        name: None,
        state: "active".into(),
        warning: None,
    });
    {
        let detail = model.session_detail_mut().unwrap();
        detail.composer.set_text("draft");
        detail.composer.focused = true;
    }
    let ctrl_c = Msg::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    let commands = update(&mut model, ctrl_c);
    assert_eq!(
        commands,
        vec![Cmd::After(Duration::from_secs(3), TimerId::QuitGuard)]
    );
    assert!(model.quit_guard);
    assert!(matches!(
        update(
            &mut model,
            Msg::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))
        )
        .last(),
        Some(Cmd::Quit)
    ));
    let mut model = Model::tail(SessionHeader::default());
    model.session_detail_mut().unwrap().composer.set_text("x");
    update(
        &mut model,
        Msg::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
    );
    update(&mut model, Msg::Timer(TimerId::QuitGuard));
    assert!(!model.quit_guard);
}
#[test]
fn ut_454_q_in_composer_is_text() {
    let mut model = Model::tail(SessionHeader::default());
    model.session_detail_mut().unwrap().composer.focused = true;
    assert!(update(&mut model, press(KeyCode::Char('q'))).is_empty());
    assert_eq!(model.session_detail().unwrap().composer.text(), "q");
}
#[test]
fn ut_420_ut_421_ut_422_ut_423_ut_424_header_states() {
    let mut model = full();
    model.workspace.as_mut().unwrap().name = "batuta-cli".into();
    model.daemon.version = Some("v0.3.0-beta.16-9-ga35eda6d".into());
    model.attention = vec![Default::default(), Default::default()];
    assert_eq!(
        header::text(&model, 120),
        "batuta · ws: batuta-cli ▾ · daemon running v0.3.0-beta.16-9-ga35eda6d · 2 attention"
    );
    model.daemon.status = "draining".into();
    assert!(header::text(&model, 120).contains("daemon draining"));
    model.workspace.as_mut().unwrap().name = "x".repeat(60);
    assert!(header::text(&model, 50).contains('…'));
    model.daemon.version = Some("dev".into());
    assert!(header::has_banner(&model));
}
#[test]
fn ut_640_ut_641_and_ut_643_stream_stale_and_offline() {
    let mut model = full();
    let catalog = StreamId::Catalog;
    let run = StreamId::RunEvents("r".into());
    model.active_streams.extend([catalog.clone(), run.clone()]);
    model.stream_status.insert(run.clone(), StreamStatus::Live);
    let lost = AnyStreamEvent::Catalog(StreamEvent::Lost {
        attempt: 1,
        next_in: Duration::from_secs(1),
        error: "lost".into(),
        offline_after: 5,
    });
    update(
        &mut model,
        Msg::Stream {
            id: catalog.clone(),
            event: lost,
        },
    );
    assert!(matches!(
        model.stream_status.get(&catalog),
        Some(StreamStatus::Stale)
    ));
    assert!(!header::offline(&model));
    model.stream_status.insert(run, StreamStatus::Stale);
    model.daemon.poll_ok = false;
    assert!(header::offline(&model));
    model.daemon.poll_ok = true;
    assert!(!header::offline(&model));
}
#[test]
fn ut_664_late_api_is_dropped() {
    let mut model = full();
    model.dirty = false;
    let commands = update(
        &mut model,
        Msg::Api {
            request: RequestId(999),
            result: Ok(ApiResponse::Empty),
        },
    );
    assert!(commands.is_empty());
    assert!(!model.dirty);
}
#[test]
fn workspace_ref_contract() {
    let workspace = WorkspaceRef {
        id: "ws".into(),
        name: "name".into(),
        root_dir: "/tmp".into(),
    };
    assert_eq!(workspace.name, "name");
}
