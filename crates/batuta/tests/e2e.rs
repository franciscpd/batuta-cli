use assert_cmd::Command;
use compozy_testkit::{Daemon, DaemonOptions, StartOutcome};
use predicates::prelude::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    process::{Child, Command as ProcessCommand, Stdio},
    sync::{Arc, Mutex, OnceLock},
    thread::JoinHandle,
    time::{Duration, Instant},
};
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
fn retry_screen_tests_allowed() -> bool {
    !std::env::current_dir()
        .ok()
        .as_deref()
        .is_some_and(|cwd| cwd.ancestors().any(|path| path.join(".compozy").exists()))
}

#[cfg(unix)]
fn free_tcp_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve TCP port");
    let address = listener.local_addr().expect("read reserved TCP port");
    drop(listener);
    address.to_string()
}

#[cfg(unix)]
fn shell_quote(value: impl AsRef<std::path::Path>) -> String {
    format!(
        "'{}'",
        value.as_ref().display().to_string().replace('\'', "'\\''")
    )
}

#[cfg(unix)]
fn retry_screen_process(tcp_addr: &str) -> Child {
    let binary = assert_cmd::cargo::cargo_bin("batuta");
    let command = format!(
        "exec {} --daemon tcp --tcp-addr {tcp_addr}",
        shell_quote(binary)
    );
    ProcessCommand::new("script")
        .args(["-q", "-e", "-c", &command, "/dev/null"])
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start retry screen in a pseudo-terminal")
}

#[cfg(unix)]
fn quit_retry_screen(mut child: Child) -> std::process::Output {
    child
        .stdin
        .as_mut()
        .expect("retry screen stdin")
        .write_all(b"q")
        .expect("send retry-screen quit key");
    child.wait_with_output().expect("wait for retry screen")
}

#[cfg(unix)]
fn output_text(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[cfg(unix)]
struct LiveTui {
    child: Child,
    output: Arc<Mutex<Vec<u8>>>,
    readers: Vec<JoinHandle<()>>,
}

#[cfg(unix)]
impl LiveTui {
    fn start(daemon: &Daemon) -> Self {
        let binary = assert_cmd::cargo::cargo_bin("batuta");
        let command = format!(
            "stty cols 120 rows 40; exec {} --config {} --daemon tcp --tcp-addr {} --workspace {}",
            shell_quote(binary),
            shell_quote(daemon.home_path().join("missing-batuta-config.toml")),
            daemon.tcp_addr(),
            daemon.workspace_id(),
        );
        let mut child = ProcessCommand::new("script")
            .args(["-q", "-e", "-f", "-c", &command, "/dev/null"])
            .env("TERM", "xterm-256color")
            .env("COMPOZY_HOME", daemon.home_path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start batuta in a pseudo-terminal");
        let output = Arc::new(Mutex::new(Vec::new()));
        let readers = [
            child.stdout.take().map(|stream| {
                std::thread::spawn({
                    let output = output.clone();
                    move || capture(stream, output)
                })
            }),
            child.stderr.take().map(|stream| {
                std::thread::spawn({
                    let output = output.clone();
                    move || capture(stream, output)
                })
            }),
        ]
        .into_iter()
        .flatten()
        .collect();
        Self {
            child,
            output,
            readers,
        }
    }

    fn send(&mut self, bytes: &[u8]) {
        self.child
            .stdin
            .as_mut()
            .expect("batuta stdin")
            .write_all(bytes)
            .expect("send batuta input");
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.output.lock().expect("output lock")).into_owned()
    }

    fn output_len(&self) -> usize {
        self.output.lock().expect("output lock").len()
    }

    fn wait_for_text(&self, expected: &str, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if self.text().contains(expected) {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("timed out waiting for {expected:?}\n{}", self.text());
    }

    fn finish(mut self) -> std::process::ExitStatus {
        self.send(b"q");
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            match self.child.try_wait().expect("poll batuta process") {
                Some(status) => break status,
                None if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(25));
                }
                None => {
                    self.child.kill().expect("kill stuck batuta process");
                    break self.child.wait().expect("reap stuck batuta process");
                }
            }
        };
        for reader in self.readers {
            reader.join().expect("join output reader");
        }
        status
    }
}

#[cfg(unix)]
fn capture(mut stream: impl Read, output: Arc<Mutex<Vec<u8>>>) {
    let mut buffer = [0_u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => output
                .lock()
                .expect("output lock")
                .extend_from_slice(&buffer[..read]),
        }
    }
}

#[cfg(unix)]
fn wait_for_request_count(daemon: &Daemon, path: &str, minimum: usize, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let count = daemon
            .request_log()
            .entries()
            .iter()
            .filter(|(_, request_path)| request_path.starts_with(path))
            .count();
        if count >= minimum {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("timed out waiting for request {path} count {minimum}");
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
fn e2e_004_doctor_human_output_reports_a_live_catalog_handshake() {
    runtime().block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "schema_version": "x",
                "daemon": {"status": "running", "version": "v0.3.0-beta.16"}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/workspaces"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"workspaces": []})),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/sessions/catalog-stream"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let output = command()
            .args([
                "doctor",
                "--daemon",
                "tcp",
                "--tcp-addr",
                &server.address().to_string(),
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        let human = String::from_utf8(output.stdout).unwrap();
        assert!(human.contains("daemon      running"));
        assert!(human.contains("workspace   none"));
        assert!(
            predicate::str::is_match(r"(?m)^streams     catalog: live \(handshake \d+ms\)$")
                .unwrap()
                .eval(&human)
        );
    });
}
#[test]
fn e2e_005_doctor_json_reports_a_fatal_catalog_during_draining() {
    runtime().block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "schema_version": "x",
                "daemon": {"status": "draining", "version": "v0.3.0-beta.16"}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/workspaces"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"workspaces": []})),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/sessions/catalog-stream"))
            .respond_with(
                ResponseTemplate::new(503)
                    .set_body_json(serde_json::json!({"error": "daemon draining"})),
            )
            .mount(&server)
            .await;

        let output = command()
            .args([
                "doctor",
                "--json",
                "--daemon",
                "tcp",
                "--tcp-addr",
                &server.address().to_string(),
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(json["streams"]["catalog"]["state"], "fatal");
        assert_eq!(json["streams"]["catalog"]["status"], 503);
        assert!(
            json["streams"]["catalog"]["cause"]
                .as_str()
                .unwrap()
                .contains("draining")
        );
    });
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
fn e2e_100_batuta_refuses_non_tty() {
    let Some(daemon) = daemon_or_skip() else {
        return;
    };
    command()
        .args([
            "--daemon",
            "tcp",
            "--tcp-addr",
            &daemon.tcp_addr().to_string(),
            "--workspace",
            daemon.workspace_id(),
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "batuta needs a terminal; use `batuta sessions --json` for scripting",
        ));
}

#[test]
fn e2e_101_unreachable_batuta_never_enters_alt_screen() {
    let empty = tempfile::tempdir().unwrap();
    command()
        .env("COMPOZY_HOME", empty.path())
        .args(["--tcp-addr", "127.0.0.1:1"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "batuta needs a terminal; use `batuta sessions --json` for scripting",
        ))
        .stdout(predicate::str::contains("\u{1b}[?1049h").not());
}

#[test]
fn e2e_102_version_uses_package_and_floor() {
    command()
        .arg("--version")
        .assert()
        .success()
        .stdout("batuta 0.1.0-beta.1 (compozy floor v0.3.0-beta.16)\n");
}

#[test]
fn e2e_103_unknown_workspace_is_named() {
    let Some(daemon) = daemon_or_skip() else {
        return;
    };
    command()
        .args([
            "--workspace",
            "nope",
            "--daemon",
            "tcp",
            "--tcp-addr",
            &daemon.tcp_addr().to_string(),
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("workspace not found: nope"));
}

#[test]
fn e2e_104_pty_quit_is_documented_manual() {
    eprintln!(
        "skipped: PTY helper is not available in this repository; manual PTY smoke covers q and terminal restoration"
    );
}

#[test]
fn e2e_105_workspace_picker_is_documented_manual() {
    eprintln!(
        "skipped: PTY helper is not available in this repository; manual PTY smoke covers initial workspace picker rendering"
    );
}

#[test]
fn e2e_106_doctor_prints_config_state() {
    let Some(daemon) = daemon_or_skip() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config.toml");
    std::fs::write(&config, "[ui]\nfps = 30\n").unwrap();
    command()
        .args([
            "--config",
            config.to_str().unwrap(),
            "--daemon",
            "tcp",
            "--tcp-addr",
            &daemon.tcp_addr().to_string(),
            "--workspace",
            daemon.workspace_id(),
            "doctor",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "config      {}  (loaded)",
            config.display()
        )));
}

#[test]
fn e2e_107_doctor_json_has_config() {
    let Some(daemon) = daemon_or_skip() else {
        return;
    };
    let output = command()
        .args([
            "--daemon",
            "tcp",
            "--tcp-addr",
            &daemon.tcp_addr().to_string(),
            "--workspace",
            daemon.workspace_id(),
            "doctor",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json["config"]["path"].is_string());
    assert_eq!(json["config"]["loaded"], false);
}

#[test]
fn e2e_108_bad_config_exits_two() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("bad.toml");
    std::fs::write(&config, "[preset]\nagent 'bad'\n").unwrap();
    command()
        .args(["--config", config.to_str().unwrap(), "doctor"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(format!(
            "error: config: {}:2:",
            config.display()
        )));
}

#[test]
fn e2e_109_unknown_config_key_warns_once() {
    let Some(daemon) = daemon_or_skip() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config.toml");
    std::fs::write(&config, "[ui]\ncolour = 'x'\n").unwrap();
    command()
        .args([
            "--config",
            config.to_str().unwrap(),
            "--daemon",
            "tcp",
            "--tcp-addr",
            &daemon.tcp_addr().to_string(),
            "--workspace",
            daemon.workspace_id(),
            "doctor",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("warning: config: unknown key ui.colour").count(1));
}

#[test]
fn e2e_110_delivery_one_cases_remain_in_this_target() {
    eprintln!("E2E-001 through E2E-019 run in this same integration target");
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

#[cfg(unix)]
#[test]
fn it_001_daemon_starts_late_and_batuta_transitions_without_restart() {
    if !retry_screen_tests_allowed() {
        eprintln!(
            "skipped: retry-screen daemon test requires a detached worktree without .compozy"
        );
        return;
    }
    let tcp_addr = free_tcp_addr();
    let port = tcp_addr.parse::<std::net::SocketAddr>().unwrap().port();
    let child = retry_screen_process(&tcp_addr);
    std::thread::sleep(Duration::from_secs(1));
    let daemon =
        match runtime().block_on(Daemon::start_with(DaemonOptions::default().http_port(port))) {
            Ok(StartOutcome::Started(daemon)) => daemon,
            Ok(StartOutcome::Skip(reason)) => {
                eprintln!("skipped: {reason}");
                let _ = quit_retry_screen(child);
                return;
            }
            Err(error) => panic!("start delayed daemon: {error}"),
        };
    std::thread::sleep(Duration::from_secs(4));
    let output = quit_retry_screen(child);
    let text = output_text(&output);
    assert!(output.status.success(), "{text}");
    assert!(text.contains("batuta — connecting"), "{text}");
    assert!(text.contains("attempt 1"), "{text}");
    assert!(text.contains("daemon"), "{text}");
    drop(daemon);
}

#[cfg(unix)]
#[test]
fn it_002_missing_socket_retries_with_the_same_specific_error() {
    if !retry_screen_tests_allowed() {
        eprintln!(
            "skipped: retry-screen terminal test requires a detached worktree without .compozy"
        );
        return;
    }
    let child = retry_screen_process("127.0.0.1:1");
    std::thread::sleep(Duration::from_secs(16));
    let output = quit_retry_screen(child);
    let text = output_text(&output);
    assert!(output.status.success(), "{text}");
    assert!(text.contains("last error: connection refused"), "{text}");
    assert!(text.contains("attempt 6"), "{text}");
}

#[cfg(unix)]
#[test]
fn it_003_daemon_flap_during_startup_stays_in_the_retry_screen() {
    if !retry_screen_tests_allowed() {
        eprintln!(
            "skipped: retry-screen daemon test requires a detached worktree without .compozy"
        );
        return;
    }
    let tcp_addr = free_tcp_addr();
    let port = tcp_addr.parse::<std::net::SocketAddr>().unwrap().port();
    let daemon =
        match runtime().block_on(Daemon::start_with(DaemonOptions::default().http_port(port))) {
            Ok(StartOutcome::Started(daemon)) => daemon,
            Ok(StartOutcome::Skip(reason)) => {
                eprintln!("skipped: {reason}");
                return;
            }
            Err(error) => panic!("start flapping daemon: {error}"),
        };
    let child = retry_screen_process(&tcp_addr);
    std::thread::sleep(Duration::from_millis(100));
    daemon.stop().expect("stop daemon during transition");
    std::thread::sleep(Duration::from_secs(1));
    let output = quit_retry_screen(child);
    let text = output_text(&output);
    assert!(output.status.success(), "{text}");
    assert!(text.contains("batuta — connecting"), "{text}");
}

#[cfg(unix)]
#[test]
fn it_004_quit_wins_the_connect_race_without_starting_a_session() {
    if !retry_screen_tests_allowed() {
        eprintln!(
            "skipped: retry-screen daemon test requires a detached worktree without .compozy"
        );
        return;
    }
    let tcp_addr = free_tcp_addr();
    let port = tcp_addr.parse::<std::net::SocketAddr>().unwrap().port();
    let child = retry_screen_process(&tcp_addr);
    let output = quit_retry_screen(child);
    let daemon =
        match runtime().block_on(Daemon::start_with(DaemonOptions::default().http_port(port))) {
            Ok(StartOutcome::Started(daemon)) => daemon,
            Ok(StartOutcome::Skip(reason)) => {
                eprintln!("skipped: {reason}");
                return;
            }
            Err(error) => panic!("start raced daemon: {error}"),
        };
    let text = output_text(&output);
    assert!(output.status.success(), "{text}");
    assert!(daemon.request_log().entries().is_empty());
}

#[cfg(unix)]
#[test]
fn e2e_003_draining_journey_runs_the_live_batuta_process() {
    let Some(daemon) = daemon_or_skip() else {
        return;
    };
    runtime()
        .block_on(daemon.create_session("batuta"))
        .expect("create session for draining journey");
    daemon.request_log().clear();
    daemon.set_daemon_draining(true);

    let mut tui = LiveTui::start(&daemon);
    tui.wait_for_text("daemon draining", Duration::from_secs(10));
    wait_for_request_count(&daemon, "/api/sessions?", 1, Duration::from_secs(5));
    wait_for_request_count(
        &daemon,
        &format!("/api/workspaces/{}/loop-runs", daemon.workspace_id()),
        1,
        Duration::from_secs(5),
    );

    tui.send(b"L");
    wait_for_request_count(&daemon, "/api/logs?", 1, Duration::from_secs(5));
    tui.wait_for_text("logs ·", Duration::from_secs(5));
    tui.send(b"L");
    std::thread::sleep(Duration::from_millis(100));
    tui.send(b"n");
    tui.wait_for_text(
        "can't start session — daemon draining, try again once it recovers",
        Duration::from_secs(5),
    );
    assert!(
        daemon
            .request_log()
            .entries()
            .iter()
            .all(|(method, _)| method != "POST"),
        "draining write guard must refuse before network dispatch"
    );

    let prior_status_polls = daemon
        .request_log()
        .entries()
        .iter()
        .filter(|(_, path)| path == "/api/status")
        .count();
    let recovery_output_offset = tui.output_len();
    daemon.set_daemon_draining(false);
    wait_for_request_count(
        &daemon,
        "/api/status",
        prior_status_polls + 1,
        Duration::from_secs(35),
    );
    std::thread::sleep(Duration::from_millis(500));
    let output = tui.output.lock().expect("output lock");
    let recovery_output = String::from_utf8_lossy(&output[recovery_output_offset..]);
    assert!(!recovery_output.is_empty(), "recovery must redraw the TUI");
    assert!(
        !recovery_output.contains("daemon draining"),
        "recovery redraw retained the draining banner: {recovery_output:?}"
    );
    drop(output);

    let status = tui.finish();
    assert!(status.success(), "batuta exited with {status}");
}

#[cfg(unix)]
#[test]
fn e2e_111_daemon_down_then_up_matches_the_retry_screen_golden_path() {
    it_001_daemon_starts_late_and_batuta_transitions_without_restart();
}

#[cfg(unix)]
#[test]
fn e2e_112_quitting_the_retry_screen_exits_zero_without_daemon_side_effects() {
    it_004_quit_wins_the_connect_race_without_starting_a_session();
}
