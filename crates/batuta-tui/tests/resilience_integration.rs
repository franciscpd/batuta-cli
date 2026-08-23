use batuta_tui::{
    AnyStreamEvent, Cmd, Model, Msg, Request, RequestId, StreamId,
    app::{AppMode, DaemonState, PendingKind, Settings, WorkspaceRef},
    runtime::{RuntimeClient, run_with_messages},
    update,
    views::{self, header},
};
use compozy_client::{
    Client, NoCursor, ReconnectPolicy, StreamEvent,
    types::{PromptMode, PromptRequest},
};
use compozy_testkit::{Daemon, StartOutcome};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use futures_util::StreamExt;
use ratatui::{Terminal, backend::TestBackend};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

async fn daemon_or_skip() -> Option<Box<Daemon>> {
    if std::env::current_dir()
        .ok()
        .as_deref()
        .is_some_and(|cwd| cwd.ancestors().any(|path| path.join(".compozy").exists()))
    {
        eprintln!("skipped: contract daemon requires a detached worktree without .compozy");
        return None;
    }
    match Daemon::start().await.expect("start disposable daemon") {
        StartOutcome::Started(daemon) => Some(daemon),
        StartOutcome::Skip(reason) => {
            eprintln!("skipped: {reason}");
            None
        }
    }
}

fn model_for(daemon: &Daemon) -> Model {
    let settings = Settings {
        workspace: Some(WorkspaceRef {
            id: daemon.workspace_id().to_owned(),
            name: "workspace".into(),
            root_dir: daemon.home_path().display().to_string(),
        }),
        ..Settings::default()
    };
    Model::new(settings, AppMode::Full)
}

fn render(model: &Model, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal
        .draw(|frame| views::view(model, frame))
        .expect("render model");
    terminal.backend().to_string()
}

fn key(code: KeyCode) -> Msg {
    Msg::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

async fn execute(model: &mut Model, client: &Client, request: Request) -> Vec<Cmd> {
    let id = request.id();
    model
        .pending
        .insert(id, PendingKind::Request(request.clone()));
    let future = if request.is_write() {
        RuntimeClient::post(client, request)
    } else {
        RuntimeClient::get(client, request)
    };
    let result = tokio::time::timeout(Duration::from_secs(2), future)
        .await
        .expect("runtime request exceeded two-second bound");
    update(
        model,
        Msg::Api {
            request: id,
            result,
        },
    )
}

async fn poll_status(model: &mut Model, client: &Client) {
    let request = model.allocate(|id| Request::Status { id });
    execute(model, client, request).await;
}

async fn wait_until(mut predicate: impl FnMut() -> bool, message: &str) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{message}"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn it_005_rapid_draining_flaps_always_render_the_latest_state() {
    let Some(daemon) = daemon_or_skip().await else {
        return;
    };
    let client = Client::tcp(daemon.tcp_addr().to_string());
    let mut model = model_for(&daemon);
    let started = Instant::now();

    for draining in [true, false, true, false, true, false] {
        daemon.set_daemon_draining(draining);
        poll_status(&mut model, &client).await;
        let expected = if draining {
            DaemonState::Draining
        } else {
            DaemonState::Connected
        };
        assert_eq!(model.daemon_state(), expected);
        assert_eq!(
            header::text(&model, 120).contains("daemon draining"),
            draining
        );
    }
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn it_006_draining_then_unreachable_never_leaves_a_stuck_banner() {
    let Some(daemon) = daemon_or_skip().await else {
        return;
    };
    let client = Client::tcp(daemon.tcp_addr().to_string());
    let mut model = model_for(&daemon);
    daemon.set_daemon_draining(true);
    poll_status(&mut model, &client).await;
    assert_eq!(model.daemon_state(), DaemonState::Draining);

    model.active_streams.insert(StreamId::Catalog);
    let (_, cursor) = tokio::sync::watch::channel(NoCursor);
    let catalog = client.catalog_stream(cursor, ReconnectPolicy::default());
    futures_util::pin_mut!(catalog);
    tokio::select! {
        () = wait_until(
            || daemon.active_catalog_connections() == 1,
            "catalog stream did not connect before daemon exit",
        ) => {}
        event = catalog.next() => panic!("catalog emitted before daemon exit: {event:?}"),
    }
    daemon.stop().expect("stop disposable daemon");
    let lost = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(event @ StreamEvent::Lost { .. }) = catalog.next().await {
                break event;
            }
        }
    })
    .await
    .expect("catalog stream did not report the daemon exit");
    update(
        &mut model,
        Msg::Stream {
            id: StreamId::Catalog,
            event: AnyStreamEvent::Catalog(lost),
        },
    );
    poll_status(&mut model, &client).await;
    assert_eq!(model.daemon_state(), DaemonState::Offline);
    assert!(!header::text(&model, 120).contains("daemon draining"));
    assert!(header::text(&model, 120).contains("daemon offline"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn it_007_in_flight_prompt_fails_clearly_when_draining_begins() {
    let Some(daemon) = daemon_or_skip().await else {
        return;
    };
    let session = daemon
        .create_session("batuta")
        .await
        .expect("create prompt target session");
    daemon.set_prompt_delay(Duration::from_millis(300));
    let client = Client::tcp(daemon.tcp_addr().to_string());
    let mut model = model_for(&daemon);
    let request = Request::Prompt {
        id: RequestId(77),
        workspace: daemon.workspace_id().to_owned(),
        session,
        prompt: PromptRequest {
            message: "continue".into(),
            message_id: "msg_it_007".into(),
            idempotency_key: "idem_it_007".into(),
            mode: PromptMode::Queue,
            expected_turn_id: None,
            runtime: None,
        },
        consume_body: false,
    };
    model.prompt_pending = true;
    model
        .pending
        .insert(request.id(), PendingKind::Request(request.clone()));
    let post = tokio::spawn(RuntimeClient::post(&client, request.clone()));
    wait_until(
        || {
            daemon
                .request_log()
                .entries()
                .iter()
                .any(|(method, path)| method == "POST" && path.ends_with("/prompt"))
        },
        "prompt request never reached the daemon proxy",
    )
    .await;
    daemon.set_daemon_draining(true);
    let result = tokio::time::timeout(Duration::from_secs(2), post)
        .await
        .expect("in-flight prompt hung after draining began")
        .expect("prompt runtime task panicked");
    update(
        &mut model,
        Msg::Api {
            request: request.id(),
            result,
        },
    );

    assert!(!model.prompt_pending);
    assert_eq!(
        model.toast.as_ref().map(|toast| toast.text.as_str()),
        Some("daemon is draining — writes refused")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn it_008_draining_with_zero_sessions_keeps_the_normal_empty_state() {
    let Some(daemon) = daemon_or_skip().await else {
        return;
    };
    daemon.set_daemon_draining(true);
    let client = Client::tcp(daemon.tcp_addr().to_string());
    let mut model = model_for(&daemon);
    poll_status(&mut model, &client).await;
    let sessions = model.allocate(|id| Request::Sessions {
        id,
        workspace: daemon.workspace_id().to_owned(),
        agent: Some("batuta".into()),
        session_type: None,
        limit: 50,
    });
    execute(&mut model, &client, sessions).await;
    let runs = model.allocate(|id| Request::Runs {
        id,
        workspace: daemon.workspace_id().to_owned(),
        loop_name: Some("batuta-deliver".into()),
        limit: 50,
    });
    execute(&mut model, &client, runs).await;

    let screen = render(&model, 100, 30);
    assert!(screen.contains("no sessions"), "{screen}");
    assert!(!screen.contains("sessions unavailable"), "{screen}");
    assert!(screen.contains("no runs for batuta-deliver"), "{screen}");
    assert!(!screen.contains("runs unavailable"), "{screen}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn it_012_workspace_switch_mid_backoff_leaves_one_runtime_stream() {
    let Some(daemon) = daemon_or_skip().await else {
        return;
    };
    daemon.set_catalog_draining(true);
    let requests = daemon.request_log();
    let client = Client::tcp(daemon.tcp_addr().to_string());
    let model = model_for(&daemon);
    let (sender, receiver) = mpsc::unbounded_channel();
    let runtime_sender = sender.clone();
    tokio::task::LocalSet::new()
        .run_until(async move {
            let runtime = tokio::task::spawn_local(async move {
                let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("test terminal");
                run_with_messages(model, client, &mut terminal, runtime_sender, receiver).await
            });

            wait_until(
                || {
                    requests.entries().iter().any(|(method, path)| {
                        method == "GET" && path == "/api/sessions/catalog-stream"
                    })
                },
                "initial catalog stream did not enter backoff",
            )
            .await;
            sender.send(key(KeyCode::Char('w'))).expect("open picker");
            wait_until(
                || {
                    requests
                        .entries()
                        .iter()
                        .any(|(method, path)| method == "GET" && path == "/api/workspaces")
                },
                "workspace picker did not load from the daemon",
            )
            .await;
            tokio::time::sleep(Duration::from_millis(50)).await;
            sender.send(key(KeyCode::Enter)).expect("select workspace");
            wait_until(
                || {
                    requests
                        .entries()
                        .iter()
                        .filter(|(method, path)| {
                            method == "GET" && path == "/api/sessions/catalog-stream"
                        })
                        .count()
                        >= 2
                },
                "replacement catalog stream did not start",
            )
            .await;

            daemon.set_catalog_draining(false);
            wait_until(
                || daemon.active_catalog_connections() == 1,
                "catalog stream did not recover to one live connection",
            )
            .await;
            tokio::time::sleep(Duration::from_millis(750)).await;
            assert_eq!(daemon.active_catalog_connections(), 1);

            sender.send(Msg::Quit).expect("quit runtime");
            tokio::time::timeout(Duration::from_secs(2), runtime)
                .await
                .expect("runtime did not stop")
                .expect("runtime task panicked")
                .expect("runtime failed");
            wait_until(
                || daemon.active_catalog_connections() == 0,
                "catalog connection leaked after runtime shutdown",
            )
            .await;
        })
        .await;
}
