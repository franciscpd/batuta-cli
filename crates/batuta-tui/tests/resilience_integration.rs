#[path = "support/panels.rs"]
mod panels_support;

use batuta_tui::{
    AnyStreamEvent, ApiResponse, Cmd, Msg, Request, StreamId,
    app::{DaemonState, StreamStatus},
    update,
    views::header,
};
use compozy_client::{ReconnectPolicy, StreamEvent, types::Workspace};
use crossterm::event::KeyCode;
use panels_support::{key, model, render, respond, runs_page, session_page};

fn lost_catalog() -> AnyStreamEvent {
    AnyStreamEvent::Catalog(StreamEvent::Lost {
        attempt: 1,
        next_in: ReconnectPolicy::default().min,
        error: "HTTP 503: daemon is draining".into(),
        offline_after: ReconnectPolicy::default().offline_after,
    })
}

#[test]
fn it_005_rapid_draining_flaps_always_render_the_latest_state() {
    let mut model = model();
    for (status, expected) in [
        ("draining", DaemonState::Draining),
        ("running", DaemonState::Connected),
        ("draining", DaemonState::Draining),
        ("running", DaemonState::Connected),
        ("draining", DaemonState::Draining),
    ] {
        model.daemon.status = status.into();
        assert_eq!(model.daemon_state(), expected);
        assert_eq!(
            header::text(&model, 120).contains("daemon draining"),
            expected == DaemonState::Draining
        );
    }
}

#[test]
fn it_006_draining_then_unreachable_never_leaves_a_stuck_banner() {
    let mut model = model();
    model.active_streams.insert(StreamId::Catalog);
    model.daemon.status = "draining".into();
    assert_eq!(model.daemon_state(), DaemonState::Draining);

    update(
        &mut model,
        Msg::Stream {
            id: StreamId::Catalog,
            event: lost_catalog(),
        },
    );
    model.daemon.poll_ok = false;
    model.daemon.status = "running".into();
    assert_eq!(model.daemon_state(), DaemonState::Offline);
    assert!(!header::text(&model, 120).contains("daemon draining"));
    assert!(header::text(&model, 120).contains("daemon offline"));
}

#[test]
fn it_007_in_flight_write_fails_clearly_when_draining_begins() {
    let mut model = model();
    let request = update(&mut model, key(KeyCode::Char('n')))
        .into_iter()
        .find_map(|command| match command {
            Cmd::Post(request @ Request::CreateSession { .. }) => Some(request),
            _ => None,
        })
        .expect("create-session request");
    model.daemon.status = "draining".into();
    let id = request.id();
    model
        .pending
        .insert(id, batuta_tui::app::PendingKind::Request(request));
    update(
        &mut model,
        Msg::Api {
            request: id,
            result: Err("HTTP 503: daemon is draining".into()),
        },
    );
    assert!(!model.create_session_pending);
    assert_eq!(
        model.toast.as_ref().map(|toast| toast.text.as_str()),
        Some("daemon is draining")
    );
}

#[test]
fn it_008_draining_with_zero_sessions_keeps_the_normal_empty_state() {
    let mut model = model();
    model.daemon.status = "draining".into();
    let screen = render(&model, 100, 30);
    assert!(screen.contains("no sessions"), "{screen}");
    assert!(!screen.contains("sessions unavailable"), "{screen}");
}

#[test]
fn it_012_workspace_switch_mid_backoff_starts_exactly_one_catalog_stream() {
    let mut model = model();
    model.active_streams.insert(StreamId::Catalog);
    update(
        &mut model,
        Msg::Stream {
            id: StreamId::Catalog,
            event: lost_catalog(),
        },
    );
    assert!(matches!(
        model.stream_status.get(&StreamId::Catalog),
        Some(StreamStatus::Stale)
    ));

    let request = update(&mut model, key(KeyCode::Char('w')))
        .into_iter()
        .find_map(|command| match command {
            Cmd::Get(request @ Request::Workspaces { .. }) => Some(request),
            _ => None,
        })
        .expect("workspace request");
    respond(
        &mut model,
        request,
        ApiResponse::Workspaces(vec![
            Workspace {
                id: "ws-test".into(),
                name: "workspace".into(),
                root_dir: "/tmp/old".into(),
                ..Workspace::default()
            },
            Workspace {
                id: "ws-next".into(),
                name: "next".into(),
                root_dir: "/tmp/next".into(),
                ..Workspace::default()
            },
        ]),
    );
    update(&mut model, key(KeyCode::Char('j')));
    let commands = update(&mut model, key(KeyCode::Enter));
    assert_eq!(
        commands
            .iter()
            .filter(|command| matches!(command, Cmd::StopStream(StreamId::Catalog)))
            .count(),
        1
    );
    assert_eq!(
        commands
            .iter()
            .filter(|command| matches!(command, Cmd::StartStream(StreamId::Catalog)))
            .count(),
        1
    );
}

#[test]
fn e2e_003_full_draining_journey_preserves_reads_refuses_writes_and_recovers() {
    let mut model = model();
    respond(
        &mut model,
        Request::Sessions {
            id: batuta_tui::RequestId(1),
            workspace: "ws-test".into(),
            agent: Some("batuta".into()),
            limit: 50,
        },
        ApiResponse::Sessions(Box::new(session_page())),
    );
    respond(
        &mut model,
        Request::Runs {
            id: batuta_tui::RequestId(2),
            workspace: "ws-test".into(),
            loop_name: Some("batuta-deliver".into()),
            limit: 50,
        },
        ApiResponse::Runs(Box::new(runs_page(true))),
    );
    model.daemon.status = "draining".into();
    let screen = render(&model, 120, 40);
    for visible in ["daemon draining", "spike plan", "live 1 · done 1"] {
        assert!(screen.contains(visible), "missing {visible:?}\n{screen}");
    }

    let log_request = update(&mut model, key(KeyCode::Char('L')))
        .into_iter()
        .find_map(|command| match command {
            Cmd::Get(request @ Request::Logs { .. }) => Some(request),
            _ => None,
        })
        .expect("logs request");
    respond(&mut model, log_request, ApiResponse::Logs(Vec::new()));
    assert!(render(&model, 120, 40).contains("logs ·"));
    update(&mut model, key(KeyCode::Esc));

    assert!(update(&mut model, key(KeyCode::Char('n'))).is_empty());
    assert_eq!(
        model.toast.as_ref().map(|toast| toast.text.as_str()),
        Some("can't start session — daemon draining, try again once it recovers")
    );
    model.daemon.status = "running".into();
    assert!(!header::text(&model, 120).contains("daemon draining"));
}
