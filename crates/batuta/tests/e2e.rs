use assert_cmd::Command;
use compozy_testkit::{Daemon, StartOutcome};
use predicates::prelude::*;
use std::sync::OnceLock;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("create E2E runtime")
    })
}

fn daemon_or_skip() -> Option<Box<Daemon>> {
    if std::env::current_dir()
        .ok()
        .as_deref()
        .is_some_and(|cwd| cwd.ancestors().any(|path| path.join(".compozy").exists()))
    {
        eprintln!("skipped: contract daemon requires a detached worktree without .compozy");
        return None;
    }
    match runtime()
        .block_on(Daemon::start())
        .expect("start test daemon")
    {
        StartOutcome::Started(daemon) => Some(daemon),
        StartOutcome::Skip(reason) => {
            eprintln!("skipped: {reason}");
            None
        }
    }
}

fn command() -> Command {
    Command::cargo_bin("batuta").expect("batuta binary")
}

#[cfg(unix)]
fn uds_home(daemon: &Daemon) {
    std::os::unix::fs::symlink(daemon.socket_path(), daemon.home_path().join("daemon.sock"))
        .expect("link daemon socket");
}

#[test]
fn e2e_001_doctor_uses_uds() {
    let Some(daemon) = daemon_or_skip() else {
        return;
    };
    uds_home(&daemon);
    command()
        .env("COMPOZY_HOME", daemon.home_path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("transport   uds"));
}
#[test]
fn e2e_002_malformed_status_is_an_error() {
    runtime().block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/status"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;
        command()
            .args([
                "--daemon",
                "tcp",
                "--tcp-addr",
                &server.address().to_string(),
                "doctor",
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains(
                "unexpected status payload (HTTP 200)",
            ));
    });
}
#[test]
fn e2e_003_tcp_fallback_reports_missing_uds() {
    let Some(daemon) = daemon_or_skip() else {
        return;
    };
    let empty = tempfile::tempdir().unwrap();
    command()
        .env("COMPOZY_HOME", empty.path())
        .args([
            "--daemon",
            "tcp",
            "--tcp-addr",
            &daemon.tcp_addr().to_string(),
            "doctor",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("transport   tcp"));
}
#[test]
fn e2e_004_consecutive_doctors_are_independent() {
    let Some(daemon) = daemon_or_skip() else {
        return;
    };
    uds_home(&daemon);
    let first = command()
        .env("COMPOZY_HOME", daemon.home_path())
        .output()
        .unwrap();
    let second = command()
        .env("COMPOZY_HOME", daemon.home_path())
        .output()
        .unwrap();
    assert_eq!(first.status.success(), second.status.success());
}
#[test]
fn e2e_005_doctor_json_is_one_line() {
    let Some(daemon) = daemon_or_skip() else {
        return;
    };
    uds_home(&daemon);
    let output = command()
        .env("COMPOZY_HOME", daemon.home_path())
        .args(["doctor", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        std::str::from_utf8(&output.stdout).unwrap().lines().count(),
        1
    );
}
#[test]
fn e2e_006_json_unreachable() {
    let empty = tempfile::tempdir().unwrap();
    command()
        .env("COMPOZY_HOME", empty.path())
        .args(["doctor", "--json", "--tcp-addr", "127.0.0.1:1"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"ok\":false"));
}
#[test]
fn e2e_007_human_unreachable_has_hint() {
    command()
        .args(["doctor", "--daemon", "tcp", "--tcp-addr", "127.0.0.1:1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("start it with: compozy start"));
}
#[test]
fn e2e_008_forced_tcp_names_target() {
    command()
        .args(["doctor", "--daemon", "tcp", "--tcp-addr", "127.0.0.1:1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("tcp  127.0.0.1:1"));
}
#[test]
fn e2e_009_dev_warning_is_on_stderr() {
    runtime().block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/api/status")).respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"schema_version":"x","daemon":{"status":"running","version":"dev"}}))).mount(&server).await;
        Mock::given(method("GET")).and(path("/api/workspaces")).respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"workspaces":[]}))).mount(&server).await;
        command().args(["doctor", "--daemon", "tcp", "--tcp-addr", &server.address().to_string()]).assert().success().stderr(predicate::str::contains("warning: daemon version dev — compatibility unverified (floor v0.3.0-beta.16)"));
    });
}
#[test]
fn e2e_010_missing_workspaces_route_is_reported() {
    runtime().block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/api/status")).respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"schema_version":"x","daemon":{"status":"running","version":"v0.3.0-beta.16"}}))).mount(&server).await;
        Mock::given(method("GET")).and(path("/api/workspaces")).respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({"error":"route missing"}))).mount(&server).await;
        command().args(["sessions", "--daemon", "tcp", "--tcp-addr", &server.address().to_string()]).assert().failure().stderr(predicate::str::contains("route missing in this daemon version: GET /api/workspaces"));
    });
}
#[test]
fn e2e_011_workspace_lookup_does_not_resolve() {
    let Some(daemon) = daemon_or_skip() else {
        return;
    };
    command()
        .args([
            "sessions",
            "--daemon",
            "tcp",
            "--tcp-addr",
            &daemon.tcp_addr().to_string(),
            "--workspace",
            "nope",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("workspace not found: nope"));
    assert!(
        daemon
            .request_log()
            .entries()
            .iter()
            .all(|(_, path)| !path.contains("/resolve"))
    );
}
#[test]
fn e2e_012_sessions_lists_created_session() {
    let Some(daemon) = daemon_or_skip() else {
        return;
    };
    let id = runtime().block_on(daemon.create_session("batuta")).unwrap();
    command()
        .args([
            "sessions",
            "--daemon",
            "tcp",
            "--tcp-addr",
            &daemon.tcp_addr().to_string(),
            "--workspace",
            daemon.workspace_id(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(id));
}
#[test]
fn e2e_013_limit_zero_is_usage_error() {
    command()
        .args(["sessions", "--limit", "0"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--limit must be at least 1"));
}
#[test]
fn e2e_014_sessions_json_has_page() {
    let Some(daemon) = daemon_or_skip() else {
        return;
    };
    runtime().block_on(daemon.create_session("batuta")).unwrap();
    command()
        .args([
            "sessions",
            "--json",
            "--daemon",
            "tcp",
            "--tcp-addr",
            &daemon.tcp_addr().to_string(),
            "--workspace",
            daemon.workspace_id(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"page\""));
}

#[test]
fn e2e_015_tail_without_sessions_never_enters_alt_screen() {
    let Some(daemon) = daemon_or_skip() else {
        return;
    };
    command()
        .args([
            "tail",
            "--daemon",
            "tcp",
            "--tcp-addr",
            &daemon.tcp_addr().to_string(),
            "--workspace",
            daemon.workspace_id(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no batuta session in workspace"))
        .stdout(predicate::str::contains("\u{1b}[?1049h").not());
}

#[test]
fn e2e_016_missing_explicit_session_is_named() {
    let Some(daemon) = daemon_or_skip() else {
        return;
    };
    command()
        .args([
            "tail",
            "--session",
            "sess-0000000000000000",
            "--daemon",
            "tcp",
            "--tcp-addr",
            &daemon.tcp_addr().to_string(),
            "--workspace",
            daemon.workspace_id(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("session not found in workspace"));
}

#[test]
fn e2e_017_short_session_id_is_usage_error_without_daemon() {
    command()
        .args(["tail", "--session", "807cee97"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--session needs the full id"));
}

#[test]
fn e2e_018_existing_session_refuses_non_tty_stdout() {
    let Some(daemon) = daemon_or_skip() else {
        return;
    };
    let id = runtime().block_on(daemon.create_session("batuta")).unwrap();
    command()
        .args([
            "tail",
            "--session",
            &id,
            "--daemon",
            "tcp",
            "--tcp-addr",
            &daemon.tcp_addr().to_string(),
            "--workspace",
            daemon.workspace_id(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("tail needs a terminal"));
}

#[test]
fn e2e_019_fallback_tail_selection_is_get_only() {
    let Some(daemon) = daemon_or_skip() else {
        return;
    };
    let id = runtime().block_on(daemon.create_session("batuta")).unwrap();
    daemon.request_log().clear();
    command()
        .args([
            "tail",
            "--session",
            &id,
            "--daemon",
            "tcp",
            "--tcp-addr",
            &daemon.tcp_addr().to_string(),
            "--workspace",
            daemon.workspace_id(),
        ])
        .assert()
        .failure();
    let requests = daemon.request_log().entries();
    assert!(!requests.is_empty());
    assert!(requests.iter().all(|(method, _)| method == "GET"));
}

#[test]
fn ut_111_file_logging_is_opt_in() {
    let temp = tempfile::tempdir().unwrap();
    let enabled = temp.path().join("enabled.log");
    command()
        .env("BATUTA_LOG", "debug")
        .env("BATUTA_LOG_FILE", &enabled)
        .arg("tail")
        .assert()
        .failure();
    assert!(
        std::fs::read_to_string(&enabled)
            .unwrap()
            .contains("batuta command")
    );
    let disabled = temp.path().join("disabled.log");
    command()
        .env_remove("BATUTA_LOG")
        .env("BATUTA_LOG_FILE", &disabled)
        .arg("tail")
        .assert()
        .failure();
    assert!(!disabled.exists());
}
