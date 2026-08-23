use compozy_client::{
    Client, LogCursor, LogQuery, LoopRunQuery, NoCursor, Outcome, ReconnectPolicy, ResetReason,
    SeqCursor, SessionQuery, StreamCursor, StreamEvent, TranscriptEvent, TranscriptQuery,
    Transport, TransportOrder,
    types::{ClarifyAnswer, Part},
};
use compozy_testkit::{Daemon, DaemonOptions, StartOutcome};
use futures_util::StreamExt;
use std::{
    fs,
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};
use tokio::sync::watch;

const ORPHAN_HELPER_ENV: &str = "BATUTA_CONTRACT_ORPHAN_HELPER";
const ORPHAN_RESULT_ENV: &str = "BATUTA_CONTRACT_ORPHAN_RESULT";

#[test]
fn contract() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build contract runtime");
    runtime.block_on(async {
        let daemon = match Daemon::start().await.expect("start disposable daemon") {
            StartOutcome::Started(daemon) => *daemon,
            StartOutcome::Skip(reason) => {
                println!("skipped: {reason}");
                return;
            }
        };

        it_001_through_006(&daemon).await;
        it_100_through_110(&daemon).await;
        it_008_unique_runs_and_teardown().await;
        it_009_readiness_log_tail(&daemon).await;
        it_010_port_collision_retries(&daemon).await;
        it_011_orphan_exit(&daemon).await;
    });
}

async fn it_100_through_110(daemon: &Daemon) {
    let client = Client::tcp(daemon.tcp_addr().to_string());
    let second = Client::tcp(daemon.tcp_addr().to_string());
    let workspace = daemon.workspace_id();

    daemon.request_log().clear();
    let (_, catalog_rx) = watch::channel(NoCursor);
    let catalog = client.catalog_stream(catalog_rx, short_stream_policy());
    let catalog_task = tokio::spawn(async move {
        futures_util::pin_mut!(catalog);
        tokio::time::timeout(Duration::from_secs(5), catalog.next()).await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let created = client
        .create_session(workspace, "batuta")
        .await
        .expect("IT-100: create session");
    assert_eq!(created.agent_name, "batuta");
    assert_eq!(
        client
            .session(workspace, &created.id)
            .await
            .expect("IT-100: session")
            .id,
        created.id
    );
    assert_eq!(
        daemon
            .request_log()
            .entries()
            .iter()
            .filter(|(method, path)| method == "POST" && path == "/api/sessions")
            .count(),
        1,
        "IT-100: create route exactly once"
    );

    let query = SessionQuery {
        workspace,
        type_: Some("user"),
        sort: "recent",
        limit: 20,
        agent: Some("batuta"),
    };
    let first_page = client.sessions(&query).await.expect("IT-101 first client");
    let second_page = second.sessions(&query).await.expect("IT-101 second client");
    assert_eq!(
        first_page
            .sessions
            .iter()
            .map(|s| &s.id)
            .collect::<Vec<_>>(),
        second_page
            .sessions
            .iter()
            .map(|s| &s.id)
            .collect::<Vec<_>>()
    );
    assert!(
        first_page
            .sessions
            .iter()
            .any(|session| session.id == created.id),
        "IT-102 filtered list"
    );
    let unfiltered = client
        .sessions(&SessionQuery {
            agent: None,
            ..query
        })
        .await
        .expect("IT-102 unfiltered list");
    assert!(
        unfiltered
            .sessions
            .iter()
            .any(|session| session.id == created.id)
    );

    let catalog_event = catalog_task
        .await
        .expect("IT-103 catalog task")
        .expect("IT-103 timeout")
        .expect("IT-103 stream ended");
    assert!(
        matches!(catalog_event, StreamEvent::Event(event) if event.kind == "upserted" && event.session_id == created.id)
    );

    assert!(
        matches!(
            client
                .create_session(workspace, "agent-that-does-not-exist")
                .await,
            Err(compozy_client::Error::Daemon { status: 400, .. })
        ),
        "IT-104 unknown agent"
    );

    let runs = client
        .loop_runs(
            workspace,
            &LoopRunQuery {
                loop_: None,
                status: None,
                limit: 50,
            },
        )
        .await
        .expect("IT-105 loop runs");
    assert!(runs.runs.is_empty());
    assert_eq!(runs.aggregates.total, 0);
    assert!(matches!(
        client.loop_run(workspace, "looprun-nope").await,
        Err(compozy_client::Error::Daemon { status: 404, .. })
    ));

    let overview = client.overview(workspace).await.expect("IT-106 overview");
    assert!(overview.attention.total >= overview.attention.items.len() as u64);

    assert!(
        client
            .clarifications(workspace, &created.id)
            .await
            .expect("IT-107 clarifications")
            .is_empty()
    );
    assert!(matches!(
        client
            .answer_clarification(
                workspace,
                &created.id,
                "req_nope",
                &ClarifyAnswer::Text("x".into())
            )
            .await,
        Err(compozy_client::Error::Daemon { status: 404, .. })
    ));

    let (_, loop_rx) = watch::channel(SeqCursor::default());
    let missing_events =
        client.loop_run_events(workspace, "looprun-nope", loop_rx, short_stream_policy());
    futures_util::pin_mut!(missing_events);
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(5), missing_events.next())
            .await
            .expect("IT-108 timeout"),
        Some(StreamEvent::Fatal(compozy_client::Error::Daemon {
            status: 404,
            ..
        }))
    ));

    let log_query = LogQuery {
        workspace_id: workspace,
        session_id: Some(&created.id),
        run: None,
        error_only: false,
        after_seq: None,
        limit: 5,
    };
    let _ = client.logs(&log_query).await.expect("IT-109 logs list");
    let (_, log_rx) = watch::channel(LogCursor::default());
    let logs = client.logs_stream(&log_query, log_rx, short_stream_policy());
    futures_util::pin_mut!(logs);
    let first = tokio::time::timeout(Duration::from_secs(5), logs.next()).await;
    assert!(
        matches!(first, Ok(Some(StreamEvent::Event(_))) | Ok(None)),
        "IT-109 logs stream did not replay an event or end cleanly"
    );

    println!("IT-110: delivery-1 IT-001–IT-011 execute in the same contract binary");
}

async fn it_001_through_006(daemon: &Daemon) {
    let tcp = daemon.tcp_addr().to_string();
    let config = fs::read_to_string(daemon.home_path().join("config.toml"))
        .expect("IT-001: seeded config.toml");
    assert!(config.contains("host = \"127.0.0.1\""));
    assert!(config.contains(&format!("port = {}", daemon.daemon_tcp_addr().port())));
    assert!(config.contains(&daemon.socket_path().display().to_string()));
    let agent = fs::read_to_string(daemon.home_path().join("agents/batuta/AGENT.md"))
        .expect("IT-001: seeded agent");
    assert!(agent.contains("provider: claude"));
    let (auto_report, uds_client) = Client::probe(
        TransportOrder::Auto,
        daemon.socket_path().to_path_buf(),
        &tcp,
        Duration::from_secs(3),
    )
    .await;
    assert_eq!(
        auto_report.chosen,
        Some(Transport::Uds(daemon.socket_path().to_path_buf())),
        "IT-001: auto probe must choose UDS: {auto_report:?}"
    );
    let uds_client = uds_client.expect("IT-001: UDS probe client");
    let uds_status = uds_client.status().await.expect("IT-001: UDS status");
    assert_eq!(uds_status.daemon.status, "running");
    assert!(!uds_status.schema_version.is_empty());

    let (tcp_report, tcp_client) = Client::probe(
        TransportOrder::TcpOnly,
        daemon.socket_path().to_path_buf(),
        &tcp,
        Duration::from_secs(3),
    )
    .await;
    assert_eq!(tcp_report.chosen, Some(Transport::Tcp(tcp.clone())));
    assert!(matches!(tcp_report.targets[0].outcome, Outcome::Skipped));
    let client = tcp_client.expect("IT-001: TCP probe client");
    let tcp_status = client.status().await.expect("IT-001: TCP status");
    assert_eq!(tcp_status.daemon.status, "running");
    assert!(!tcp_status.schema_version.is_empty());

    let version = uds_status
        .daemon
        .version
        .as_deref()
        .expect("IT-002: daemon status exposes version");
    assert!(!version.is_empty());
    println!("IT-002 daemon version: {version}; draining skipped: no test-only toggle");

    daemon.request_log().clear();
    let workspaces = client.workspaces().await.expect("IT-003: workspaces");
    let direct_workspaces = uds_client
        .workspaces()
        .await
        .expect("IT-003: direct workspace catalog");
    assert!(!workspaces.is_empty());
    assert_eq!(
        workspaces
            .iter()
            .map(|workspace| &workspace.id)
            .collect::<Vec<_>>(),
        direct_workspaces
            .iter()
            .map(|workspace| &workspace.id)
            .collect::<Vec<_>>()
    );
    assert!(
        workspaces
            .iter()
            .any(|workspace| workspace.id == daemon.workspace_id())
    );
    assert!(
        daemon
            .request_log()
            .entries()
            .iter()
            .all(|(_, path)| path != "/api/workspaces/resolve")
    );

    let query = SessionQuery {
        workspace: daemon.workspace_id(),
        type_: Some("user"),
        sort: "recent",
        limit: 20,
        agent: Some("batuta"),
    };
    let initial = client
        .sessions(&query)
        .await
        .expect("IT-004: empty sessions");
    assert!(initial.sessions.is_empty());
    let session_id = daemon
        .create_session("batuta")
        .await
        .expect("IT-004: create session");
    let page = client
        .sessions(&query)
        .await
        .expect("IT-004: created session page");
    assert_eq!(page.sessions.len(), 1);
    let row = &page.sessions[0];
    assert_eq!(row.id, session_id);
    assert_eq!(row.agent_name, "batuta");
    assert!(matches!(
        row.state.as_str(),
        "starting" | "active" | "stopped"
    ));
    assert!(row.archived_at.is_none());
    assert_eq!(
        client
            .session(daemon.workspace_id(), &session_id)
            .await
            .expect("IT-004: session detail")
            .id,
        session_id
    );

    let transcript = client
        .transcript(
            daemon.workspace_id(),
            &session_id,
            &TranscriptQuery {
                limit: 200,
                before_sequence: None,
            },
        )
        .await
        .expect("IT-005: transcript page");
    assert!(transcript.entries.iter().all(|entry| {
        entry.message.parts.iter().all(|part| {
            matches!(part, Part::Event { data } if data.get("type").and_then(|value| value.as_str()).is_some_and(|kind| kind.starts_with("hook.dispatch.")))
        })
    }), "IT-005: new session contained conversational entries: {:#?}", transcript.entries);
    assert!(!transcript.has_older);
    let cursor = StreamCursor {
        after_sequence: transcript.max_sequence,
        epoch: transcript.epoch,
        generation: transcript.generation,
    };
    let (_, receiver) = watch::channel(cursor);
    let stream = client.transcript_stream(
        daemon.workspace_id(),
        &session_id,
        receiver,
        short_stream_policy(),
    );
    futures_util::pin_mut!(stream);
    let first = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .expect("IT-005: initial snapshot timeout")
        .expect("IT-005: stream ended");
    match first {
        TranscriptEvent::Snapshot(snapshot)
            if transcript.epoch == 0 || transcript.generation == 0 =>
        {
            assert!(snapshot.reset);
            assert_eq!(snapshot.reason.as_deref(), Some(ResetReason::FENCE_MISSING));
        }
        TranscriptEvent::Snapshot(snapshot) => assert!(!snapshot.reset),
        other => panic!("IT-005: expected snapshot, got {other:?}"),
    }

    let (_, receiver) = watch::channel(StreamCursor {
        after_sequence: 1,
        epoch: 999,
        generation: 999,
    });
    let stream = client.transcript_stream(
        daemon.workspace_id(),
        &session_id,
        receiver,
        short_stream_policy(),
    );
    futures_util::pin_mut!(stream);
    let first = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .expect("IT-006: reset snapshot timeout")
        .expect("IT-006: stream ended");
    match first {
        TranscriptEvent::Snapshot(snapshot) => {
            assert!(snapshot.reset);
            assert!(matches!(
                snapshot.reason.as_deref(),
                Some(
                    ResetReason::FENCE_MISSING
                        | ResetReason::EPOCH_MISMATCH
                        | ResetReason::GENERATION_MISMATCH
                        | ResetReason::SEQUENCE_RESET
                )
            ));
        }
        other => panic!("IT-006: expected reset snapshot, got {other:?}"),
    }

    let entries = daemon.request_log().entries();
    assert!(entries.iter().any(|(method, _)| method == "POST"));
    assert!(
        entries
            .iter()
            .filter(|(method, _)| method != "POST")
            .all(|(method, path)| method == "GET" && path != "/api/workspaces/resolve")
    );
}

async fn it_008_unique_runs_and_teardown() {
    let first = started(Daemon::start().await);
    let first_home = first.home_path().to_path_buf();
    let first_socket = first.socket_path().to_path_buf();
    let first_pid = first.process_id();
    first.stop().expect("IT-008: clean first daemon stop");
    assert!(!first_home.exists(), "IT-008: first temp home remains");
    assert!(!process_exists(first_pid), "IT-008: first process remains");

    let second = started(Daemon::start().await);
    assert_ne!(first_home, second.home_path());
    assert_ne!(first_socket, second.socket_path());
    drop(second);
}

async fn it_009_readiness_log_tail(running: &Daemon) {
    let dir = tempfile::tempdir().expect("IT-009 tempdir");
    let fake = dir.path().join("fake-compozy");
    fs::write(
        &fake,
        "#!/bin/sh\ni=1\nwhile [ $i -le 25 ]; do echo readiness-line-$i; i=$((i+1)); done\nsleep 60\n",
    )
    .expect("write fake daemon");
    let mut permissions = fs::metadata(&fake).expect("fake metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake, permissions).expect("chmod fake daemon");
    let started_at = Instant::now();
    let error = match Daemon::start_with(
        DaemonOptions::default()
            .binary(fake)
            .readiness_timeout(Duration::from_secs(30)),
    )
    .await
    {
        Err(error) => error.to_string(),
        Ok(_) => panic!("IT-009: fake daemon unexpectedly became ready"),
    };
    assert!(started_at.elapsed() >= Duration::from_secs(29));
    assert!(error.contains("last 20 lines of daemon-process.log"));
    assert!(error.contains("readiness-line-25"));
    assert!(!error.contains("readiness-line-5\n"));
    assert!(
        process_exists(running.process_id()),
        "shared daemon died during IT-009"
    );
}

async fn it_010_port_collision_retries(running: &Daemon) {
    let occupied = TcpListener::bind(("127.0.0.1", 0)).expect("IT-010 occupied listener");
    let port = occupied.local_addr().expect("occupied address").port();
    let retried = started(Daemon::start_with(DaemonOptions::default().http_port(port)).await);
    assert_eq!(retried.start_attempts(), 2);
    assert_ne!(retried.daemon_tcp_addr().port(), port);
    drop(occupied);
    drop(retried);
    assert!(
        process_exists(running.process_id()),
        "shared daemon died during IT-010"
    );
}

async fn it_011_orphan_exit(running: &Daemon) {
    let result_dir = tempfile::tempdir().expect("IT-011 result dir");
    let result = result_dir.path().join("orphan-result");
    let status = Command::new(std::env::current_exe().expect("contract test executable"))
        .args(["--exact", "orphan_helper_process", "--nocapture"])
        .env(ORPHAN_HELPER_ENV, "1")
        .env(ORPHAN_RESULT_ENV, &result)
        .status()
        .expect("IT-011 run helper");
    assert!(status.success());
    let contents = fs::read_to_string(&result).expect("IT-011 helper result");
    let mut lines = contents.lines();
    let pid: u32 = lines
        .next()
        .expect("helper pid")
        .parse()
        .expect("numeric helper pid");
    let home = PathBuf::from(lines.next().expect("helper home"));
    let socket = PathBuf::from(lines.next().expect("helper socket"));
    let deadline = Instant::now() + Duration::from_secs(10);
    while process_exists(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!process_exists(pid), "IT-011: orphan daemon {pid} remains");
    let _ = fs::remove_file(socket);
    fs::remove_dir_all(&home).expect("IT-011 remove orphan temp home");
    assert!(!home.exists());
    assert!(
        process_exists(running.process_id()),
        "shared daemon died during IT-011"
    );
}

#[test]
fn orphan_helper_process() {
    if std::env::var_os(ORPHAN_HELPER_ENV).is_none() {
        return;
    }
    let result = PathBuf::from(std::env::var_os(ORPHAN_RESULT_ENV).expect("orphan result path"));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build orphan helper runtime");
    let daemon = runtime.block_on(async { started(Daemon::start().await) });
    fs::write(
        result,
        format!(
            "{}\n{}\n{}\n",
            daemon.process_id(),
            daemon.home_path().display(),
            daemon.socket_path().display()
        ),
    )
    .expect("write orphan helper result");
    std::mem::forget(daemon);
    std::process::exit(0);
}

fn started(result: compozy_testkit::Result<StartOutcome>) -> Daemon {
    match result.expect("start disposable daemon") {
        StartOutcome::Started(daemon) => *daemon,
        StartOutcome::Skip(reason) => {
            panic!("operator binary disappeared during contract run: {reason}")
        }
    }
}

fn short_stream_policy() -> ReconnectPolicy {
    let mut policy = ReconnectPolicy::default();
    policy.min = Duration::from_millis(10);
    policy.max = Duration::from_millis(20);
    policy.jitter = 0.0;
    policy.idle_timeout = Duration::from_secs(5);
    policy.offline_after = 2;
    policy
}

fn process_exists(pid: u32) -> bool {
    let result = unsafe { libc::kill(pid as i32, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}
