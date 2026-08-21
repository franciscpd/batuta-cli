use batuta_tui::{
    app::{AppMode, DaemonState, DaemonStatus, Model, SessionRow, Settings, StreamStatus},
    cmd::StreamId,
    msg::{AnyStreamEvent, Msg},
    update,
    views::{self, header},
};
use compozy_client::{Error, StreamEvent};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

fn full() -> Model {
    Model::new(Settings::default(), AppMode::Full)
}

fn press(code: KeyCode) -> Msg {
    Msg::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn status(value: &str) -> DaemonStatus {
    DaemonStatus {
        status: value.into(),
        version: None,
        poll_ok: true,
    }
}

fn buffer_text(terminal: &Terminal<TestBackend>, width: u16, height: u16) -> String {
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            let mut line = String::new();
            for x in 0..width {
                line.push_str(buffer[(x, y)].symbol());
            }
            line.trim_end().to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn session_row() -> SessionRow {
    SessionRow {
        id: "s1".into(),
        agent: "batuta".into(),
        name: Some("demo".into()),
        state: "active".into(),
        badge: Some("idle".into()),
        last_activity_at: None,
    }
}

// UT-001..UT-004: `DaemonState::derive` transitions and precedence.
#[test]
fn ut_001_derive_draining() {
    assert_eq!(
        DaemonState::derive(&status("draining"), false),
        DaemonState::Draining
    );
}
#[test]
fn ut_002_derive_offline() {
    assert_eq!(
        DaemonState::derive(&status("ok"), true),
        DaemonState::Offline
    );
}
#[test]
fn ut_003_derive_connected() {
    assert_eq!(
        DaemonState::derive(&status("ok"), false),
        DaemonState::Connected
    );
}
#[test]
fn ut_004_derive_draining_takes_precedence_over_offline() {
    assert_eq!(
        DaemonState::derive(&status("draining"), true),
        DaemonState::Draining
    );
}

// UT-005/UT-006: header banner render and clear-on-recovery.
#[test]
fn ut_005_header_draining_banner_is_distinct_from_offline() {
    let mut model = full();
    model.daemon.status = "draining".into();
    model.daemon.poll_ok = true;
    let draining_text = header::text(&model, 120);
    assert!(draining_text.contains("draining"));
    assert!(!draining_text.contains("daemon offline"));
}
#[test]
fn ut_006_header_banner_clears_on_recovery() {
    let mut model = full();
    model.daemon.status = "draining".into();
    model.daemon.poll_ok = true;
    assert!(header::text(&model, 120).contains("draining"));
    model.daemon.status = "ok".into();
    assert!(!header::text(&model, 120).contains("draining"));
}

// UT-007: write-action dispatch refuses with the `_dx.md` message while draining.
#[test]
fn ut_007_create_session_refused_while_draining() {
    let mut model = full();
    model.daemon.status = "draining".into();
    model.daemon.poll_ok = true;
    let commands = update(&mut model, press(KeyCode::Char('n')));
    assert!(commands.is_empty());
    assert!(!model.create_session_pending);
    let toast = model.toast.as_ref().expect("draining refusal must toast");
    assert_eq!(
        toast.text,
        "can't start session — daemon draining, try again once it recovers"
    );
}

// UT-008: sessions list render is unaffected by draining (reads keep working).
#[test]
fn ut_008_sessions_read_view_unaffected_by_draining() {
    let mut model = full();
    model.sessions.set_items(vec![session_row()]);
    let render = |model: &Model| {
        let mut terminal = Terminal::new(TestBackend::new(60, 10)).unwrap();
        let offline = header::offline(model);
        terminal
            .draw(|frame| views::sessions::render(model, frame, frame.area(), offline))
            .unwrap();
        buffer_text(&terminal, 60, 10)
    };
    let connected = render(&model);
    model.daemon.status = "draining".into();
    model.daemon.poll_ok = true;
    let draining = render(&model);
    assert_eq!(connected, draining);
}

// UT-010: a 503 `ConnectFailure` for `StreamId::Catalog` no longer matches a
// special-cased branch (the deleted `stream.rs:62-71` block); it falls
// through to the same generic retry-status handling as other stream IDs.
#[test]
fn ut_010_catalog_503_falls_through_to_generic_stream_handling() {
    let mut model = full();
    model.active_streams.insert(StreamId::Catalog);
    let commands = update(
        &mut model,
        Msg::Stream {
            id: StreamId::Catalog,
            event: AnyStreamEvent::Catalog(StreamEvent::Fatal(Error::Daemon {
                status: 503,
                message: "daemon draining".into(),
                code: None,
                details: None,
                diagnostic: None,
            })),
        },
    );
    assert!(commands.is_empty());
    assert_eq!(
        model.stream_status.get(&StreamId::Catalog),
        Some(&StreamStatus::Stale)
    );
}
