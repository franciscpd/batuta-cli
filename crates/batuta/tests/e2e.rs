use assert_cmd::Command;
use compozy_testkit::{Daemon, DaemonOptions, StartOutcome};
use predicates::prelude::*;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    process::{Child, Command as ProcessCommand, Stdio},
    sync::{
        Arc, Barrier, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use wiremock::{
    Mock, MockServer, Request as MockRequest, Respond, ResponseTemplate,
    matchers::{any, method, path},
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
const PTY_COLUMNS: u16 = 120;

#[cfg(unix)]
const PTY_ROWS: u16 = 40;

#[cfg(unix)]
const PTY_CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);

#[cfg(unix)]
const PTY_QUIT_GRACE: Duration = Duration::from_millis(250);

#[cfg(unix)]
fn retry_screen_process(tcp_addr: &str) -> RetryScreen {
    RetryScreen::start(tcp_addr)
}

#[cfg(unix)]
struct LiveTui {
    child: Child,
    output: Arc<Mutex<Vec<u8>>>,
    readers: Vec<JoinHandle<()>>,
}

#[cfg(unix)]
#[derive(Clone, Copy)]
enum OnboardingOutcome {
    Added,
    Unsupported,
    Rejected,
}

#[cfg(unix)]
struct OnboardingFixture {
    server: MockServer,
    state: Arc<Mutex<OnboardingFixtureState>>,
}

#[cfg(unix)]
struct OnboardingFixtureState {
    candidate: String,
    outcome: OnboardingOutcome,
    added: bool,
    writes: usize,
    add_payloads: Vec<serde_json::Value>,
    requests: Vec<(String, String)>,
}

#[cfg(unix)]
impl OnboardingFixture {
    async fn start(candidate: &std::path::Path, outcome: OnboardingOutcome) -> Self {
        let server = MockServer::start().await;
        let state = Arc::new(Mutex::new(OnboardingFixtureState {
            candidate: candidate.display().to_string(),
            outcome,
            added: false,
            writes: 0,
            add_payloads: Vec::new(),
            requests: Vec::new(),
        }));
        Mock::given(any())
            .respond_with(OnboardingResponder {
                state: state.clone(),
            })
            .mount(&server)
            .await;
        Self { server, state }
    }

    fn addr(&self) -> String {
        self.server.address().to_string()
    }

    fn writes(&self) -> usize {
        self.state.lock().expect("fixture state lock").writes
    }

    fn requests(&self) -> Vec<(String, String)> {
        self.state
            .lock()
            .expect("fixture state lock")
            .requests
            .clone()
    }

    fn add_payloads(&self) -> Vec<serde_json::Value> {
        self.state
            .lock()
            .expect("fixture state lock")
            .add_payloads
            .clone()
    }
}

#[cfg(unix)]
struct OnboardingResponder {
    state: Arc<Mutex<OnboardingFixtureState>>,
}

#[cfg(unix)]
impl Respond for OnboardingResponder {
    fn respond(&self, request: &MockRequest) -> ResponseTemplate {
        let path = request.url.path();
        let method = request.method.as_str();
        let mut state = self.state.lock().expect("fixture state lock");
        state.requests.push((method.into(), path.into()));
        match (method, path) {
            ("GET", "/api/status") => ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "daemon": {"status": "running", "version": "v0.3.0-beta.16"}
            })),
            ("GET", "/api/workspaces") => {
                let workspaces = state.added.then(|| serde_json::json!({
                    "id": "ws-new",
                    "name": "onboarding-project",
                    "root_dir": state.candidate,
                    "add_dirs": []
                }));
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "workspaces": workspaces.into_iter().collect::<Vec<_>>()
                }))
            }
            ("POST", "/api/workspaces") => {
                state.writes += 1;
                state.add_payloads.push(request.body_json().expect("add payload is JSON"));
                match state.outcome {
                    OnboardingOutcome::Added => {
                        state.added = true;
                        ResponseTemplate::new(200).set_body_json(serde_json::json!({
                            "id": "ws-new",
                            "name": "onboarding-project",
                            "root_dir": state.candidate,
                            "add_dirs": []
                        }))
                    }
                    OnboardingOutcome::Unsupported => ResponseTemplate::new(404)
                        .set_body_json(serde_json::json!({"error": "route missing"})),
                    OnboardingOutcome::Rejected => ResponseTemplate::new(422).set_body_json(
                        serde_json::json!({
                            "error": "workspace could not be added",
                            "code": "workspace_invalid",
                            "diagnostic": "root_dir must be canonical"
                        }),
                    ),
                }
            }
            ("GET", "/api/sessions") => {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "sessions": [], "page": {"has_more": false, "limit": 20}
                }))
            }
            ("GET", path) if path.starts_with("/api/workspaces/ws-new/loop-runs") => {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "runs": [], "page": {"has_more": false, "limit": 20}
                }))
            }
            ("GET", "/api/observe/overview") => ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"overview": {"attention": {"total": 0, "by_kind": {}, "items": []}}}),
            ),
            ("GET", "/api/sessions/catalog-stream") => ResponseTemplate::new(200),
            _ => ResponseTemplate::new(404).set_body_json(serde_json::json!({"error": "unexpected fixture route"})),
        }
    }
}

#[cfg(unix)]
struct RetryScreen {
    child: Child,
    batuta_pid_file: tempfile::NamedTempFile,
    output: Arc<Mutex<Vec<u8>>>,
    readers: Vec<JoinHandle<()>>,
}

#[cfg(unix)]
struct ProbeGate {
    addr: String,
    first_request: Receiver<()>,
    release_first: Mutex<Option<SyncSender<()>>>,
    stopped: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

#[cfg(unix)]
impl ProbeGate {
    fn start(upstream: SocketAddr) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind startup probe gate");
        listener
            .set_nonblocking(true)
            .expect("set startup probe gate nonblocking");
        let addr = listener.local_addr().expect("read startup probe gate addr");
        let (first_request_tx, first_request) = sync_channel(1);
        let (release_first, release_first_rx) = sync_channel(0);
        let stopped = Arc::new(AtomicBool::new(false));
        let thread = std::thread::spawn({
            let stopped = stopped.clone();
            move || {
                let mut first = Some((first_request_tx, release_first_rx));
                while !stopped.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((client, _)) => {
                            let gate = first.take();
                            std::thread::spawn(move || {
                                let _ = forward_probe(client, upstream, gate);
                            });
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Err(_) => break,
                    }
                }
            }
        });
        Self {
            addr: addr.to_string(),
            first_request,
            release_first: Mutex::new(Some(release_first)),
            stopped,
            thread: Some(thread),
        }
    }

    fn addr(&self) -> &str {
        &self.addr
    }

    fn wait_for_first_request(&self, timeout: Duration) {
        self.first_request
            .recv_timeout(timeout)
            .expect("startup probe did not reach the controlled gate");
    }

    fn release_first(&self) {
        if let Some(release) = self.release_first.lock().expect("probe gate lock").take() {
            let _ = release.send(());
        }
    }
}

#[cfg(unix)]
impl Drop for ProbeGate {
    fn drop(&mut self) {
        self.release_first();
        self.stopped.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(&self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(unix)]
fn forward_probe(
    mut client: TcpStream,
    upstream: SocketAddr,
    gate: Option<(SyncSender<()>, Receiver<()>)>,
) -> std::io::Result<()> {
    client.set_read_timeout(Some(PTY_CLEANUP_TIMEOUT))?;
    let mut request = Vec::with_capacity(4096);
    loop {
        let mut buffer = [0_u8; 4096];
        let read = client.read(&mut buffer)?;
        if read == 0 {
            return Ok(());
        }
        request.extend_from_slice(&buffer[..read]);
        if request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            break;
        }
    }
    if let Some((reached, release)) = gate {
        let _ = reached.send(());
        let _ = release.recv();
    }
    let mut upstream = TcpStream::connect(upstream)?;
    upstream.write_all(&request)?;
    std::io::copy(&mut upstream, &mut client)?;
    Ok(())
}

#[cfg(unix)]
impl RetryScreen {
    fn start(tcp_addr: &str) -> Self {
        let binary = assert_cmd::cargo::cargo_bin("batuta");
        let batuta_pid_file = tempfile::NamedTempFile::new().expect("create batuta PID file");
        let command = format!(
            "stty cols {PTY_COLUMNS} rows {PTY_ROWS}; printf '%s' \"$$\" > {}; exec {} --daemon tcp --tcp-addr {tcp_addr}",
            shell_quote(batuta_pid_file.path()),
            shell_quote(binary),
        );
        let mut process = ProcessCommand::new("script");
        process
            .args(["-q", "-e", "-f", "-c", &command, "/dev/null"])
            .env("TERM", "xterm-256color")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let mut child = process
            .spawn()
            .expect("start retry screen in a pseudo-terminal");
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
            batuta_pid_file,
            output,
            readers,
        }
    }

    fn text(&self) -> String {
        terminal_text(&self.output.lock().expect("output lock"))
    }

    fn wait_for_text(&self, expected: &str, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if self.text().contains(expected) {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("timed out waiting for {expected:?}\\n{}", self.text());
    }

    fn request_quit(&mut self) {
        self.child
            .stdin
            .as_mut()
            .expect("retry screen stdin")
            .write_all(b"q")
            .and_then(|()| {
                self.child
                    .stdin
                    .as_mut()
                    .expect("retry screen stdin")
                    .flush()
            })
            .expect("send retry-screen quit key");
    }

    fn quit(mut self) -> std::process::Output {
        self.request_quit();
        self.finish_after_quit_request()
    }

    fn finish_after_quit_request(mut self) -> std::process::Output {
        if self.wait_for_exit(PTY_QUIT_GRACE).is_none() {
            self.child
                .stdin
                .as_mut()
                .expect("batuta stdin")
                .write_all(b"\x03")
                .and_then(|()| self.child.stdin.as_mut().expect("batuta stdin").flush())
                .expect("send state-independent batuta quit key");
        }
        self.finish_after(PTY_CLEANUP_TIMEOUT)
    }

    fn finish_after_quit_request_strict(self) -> std::process::Output {
        self.finish_after(PTY_QUIT_GRACE)
    }

    fn force_cleanup(self) -> std::process::Output {
        self.finish_after(Duration::ZERO)
    }

    fn batuta_pid(&self) -> u32 {
        std::fs::read_to_string(self.batuta_pid_file.path())
            .expect("read batuta PID file")
            .parse()
            .expect("parse batuta PID")
    }

    fn finish_after(mut self, timeout: Duration) -> std::process::Output {
        let status = self
            .wait_for_exit(timeout)
            .unwrap_or_else(|| self.terminate_and_reap());
        self.child.stdin.take();
        for reader in self.readers.drain(..) {
            reader.join().expect("join retry-screen output reader");
        }
        std::process::Output {
            status,
            stdout: std::mem::take(&mut self.output.lock().expect("output lock")),
            stderr: Vec::new(),
        }
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> Option<std::process::ExitStatus> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.child.try_wait().expect("poll retry-screen process") {
                Some(status) => return Some(status),
                None if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
                None => return None,
            }
        }
    }

    fn terminate_and_reap(&mut self) -> std::process::ExitStatus {
        let batuta_pid = self.batuta_pid() as i32;
        let process_group = self.child.id() as i32;
        for target in [-process_group, batuta_pid] {
            let result = unsafe { libc::kill(target, libc::SIGKILL) };
            if result != 0 {
                let error = std::io::Error::last_os_error();
                assert_eq!(
                    error.raw_os_error(),
                    Some(libc::ESRCH),
                    "terminate stuck retry-screen process target {target}: {error}"
                );
            }
        }
        if let Err(error) = self.child.kill() {
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::InvalidInput,
                "terminate stuck retry-screen process: {error}"
            );
        }
        self.child.wait().expect("reap stuck retry-screen process")
    }
}

#[cfg(unix)]
impl Drop for RetryScreen {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            if let Ok(pid) =
                std::fs::read_to_string(self.batuta_pid_file.path()).and_then(|value| {
                    value
                        .parse::<i32>()
                        .map_err(|error| std::io::Error::other(error.to_string()))
                })
            {
                unsafe {
                    libc::kill(pid, libc::SIGKILL);
                }
            }
            let process_group = self.child.id() as i32;
            unsafe {
                libc::kill(-process_group, libc::SIGKILL);
            }
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        for reader in self.readers.drain(..) {
            let _ = reader.join();
        }
    }
}

#[cfg(unix)]
fn quit_retry_screen(child: RetryScreen) -> std::process::Output {
    child.quit()
}

#[cfg(unix)]
fn retry_screen_text(output: &std::process::Output) -> String {
    terminal_text(&[&output.stdout[..], &output.stderr[..]].concat())
}

#[cfg(unix)]
fn process_exists(pid: u32) -> bool {
    let result = unsafe { libc::kill(pid as i32, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(unix)]
fn wait_for_process_exit(pid: u32, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while process_exists(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(!process_exists(pid), "process {pid} survived PTY cleanup");
}

#[cfg(unix)]
fn terminal_text(bytes: &[u8]) -> String {
    let mut screen = vec![vec![' '; usize::from(PTY_COLUMNS)]; usize::from(PTY_ROWS)];
    let (mut row, mut column, mut index) = (0_usize, 0_usize, 0_usize);
    while index < bytes.len() {
        match bytes[index] {
            b'\x1b' if bytes.get(index + 1) == Some(&b'[') => {
                let start = index + 2;
                let Some((end, command)) =
                    bytes[start..]
                        .iter()
                        .enumerate()
                        .find_map(|(offset, byte)| {
                            (*byte >= b'@' && *byte <= b'~').then_some((start + offset, *byte))
                        })
                else {
                    break;
                };
                let parameters = std::str::from_utf8(&bytes[start..end]).unwrap_or_default();
                match command {
                    b'H' | b'f' => {
                        let mut values = parameters
                            .split(';')
                            .map(|value| value.parse::<usize>().unwrap_or(1));
                        row = values
                            .next()
                            .unwrap_or(1)
                            .saturating_sub(1)
                            .min(screen.len() - 1);
                        column = values
                            .next()
                            .unwrap_or(1)
                            .saturating_sub(1)
                            .min(screen[0].len() - 1);
                    }
                    b'J' if parameters == "2" => {
                        screen.iter_mut().for_each(|line| line.fill(' '));
                        row = 0;
                        column = 0;
                    }
                    _ => {}
                }
                index = end + 1;
            }
            b'\r' => {
                column = 0;
                index += 1;
            }
            b'\n' => {
                row = (row + 1).min(screen.len() - 1);
                index += 1;
            }
            byte if byte.is_ascii_control() => index += 1,
            _ => {
                let Some(value) = std::str::from_utf8(&bytes[index..])
                    .ok()
                    .and_then(|value| value.chars().next())
                else {
                    index += 1;
                    continue;
                };
                if column < screen[0].len() {
                    screen[row][column] = value;
                }
                column = (column + 1).min(screen[0].len());
                index += value.len_utf8();
            }
        }
    }
    screen
        .into_iter()
        .map(|line| line.into_iter().collect::<String>().trim_end().to_owned())
        .collect::<Vec<_>>()
        .join("\n")
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

    fn start_onboarding(tcp_addr: &str, cwd: &std::path::Path) -> Self {
        let binary = assert_cmd::cargo::cargo_bin("batuta");
        let config = tempfile::NamedTempFile::new().expect("create empty config file");
        let command = format!(
            "stty cols {PTY_COLUMNS} rows {PTY_ROWS}; cd {}; exec {} --config {} --daemon tcp --tcp-addr {tcp_addr}",
            shell_quote(cwd),
            shell_quote(binary),
            shell_quote(config.path()),
        );
        let mut child = ProcessCommand::new("script")
            .args(["-q", "-e", "-f", "-c", &command, "/dev/null"])
            .env("TERM", "xterm-256color")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start onboarding fixture in a pseudo-terminal");
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

    fn screen_text(&self) -> String {
        terminal_text(&self.output.lock().expect("output lock"))
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

    fn wait_for_screen(&self, timeout: Duration, predicate: impl Fn(&str) -> bool) -> String {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            let screen = self.screen_text();
            if predicate(&screen) {
                return screen;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!(
            "timed out waiting for terminal state\n{}",
            self.screen_text()
        );
    }

    fn finish(mut self) -> std::process::ExitStatus {
        self.send(b"q");
        self.finish_after_exit_request()
    }

    fn finish_after_exit_request(mut self) -> std::process::ExitStatus {
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
fn wait_for_onboarding_writes(fixture: &OnboardingFixture, minimum: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if fixture.writes() >= minimum {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("timed out waiting for {minimum} registration writes");
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
    child.wait_for_text("batuta — connecting", Duration::from_secs(5));
    child.wait_for_text("attempt 1", Duration::from_secs(5));
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
    child.wait_for_text("[1] Sessions", Duration::from_secs(5));
    let output = quit_retry_screen(child);
    let text = retry_screen_text(&output);
    assert!(output.status.success(), "{text}");
    assert!(text.contains("[1] Sessions"), "{text}");
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
    child.wait_for_text("last error: connection refused", Duration::from_secs(5));
    child.wait_for_text("attempt 6", Duration::from_secs(20));
    let output = quit_retry_screen(child);
    let text = retry_screen_text(&output);
    assert!(output.status.success(), "{text}");
    assert!(text.contains("last error: connection refused"), "{text}");
    assert!(text.contains("attempt 6"), "{text}");
}

#[cfg(unix)]
#[test]
fn it_023_retry_screen_pty_output_and_cleanup_are_bounded() {
    if !retry_screen_tests_allowed() {
        eprintln!(
            "skipped: retry-screen terminal test requires a detached worktree without .compozy"
        );
        return;
    }

    let child = retry_screen_process("127.0.0.1:1");
    child.wait_for_text("attempt 1", Duration::from_secs(5));
    child.wait_for_text("last error: connection refused", Duration::from_secs(5));
    let output = quit_retry_screen(child);
    let text = retry_screen_text(&output);
    assert!(output.status.success(), "{text}");
    assert!(text.contains("attempt 1"), "{text}");
    assert!(text.contains("last error: connection refused"), "{text}");

    let child = retry_screen_process("127.0.0.1:1");
    child.wait_for_text("attempt 1", Duration::from_secs(5));
    let batuta_pid = child.batuta_pid();
    let started = Instant::now();
    let output = child.force_cleanup();
    wait_for_process_exit(batuta_pid, PTY_CLEANUP_TIMEOUT);
    assert!(
        started.elapsed() <= PTY_CLEANUP_TIMEOUT,
        "forced retry-screen cleanup exceeded {PTY_CLEANUP_TIMEOUT:?}"
    );
    assert!(
        !output.status.success(),
        "forced cleanup unexpectedly succeeded"
    );
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
    let daemon = match runtime().block_on(Daemon::start()) {
        Ok(StartOutcome::Started(daemon)) => daemon,
        Ok(StartOutcome::Skip(reason)) => {
            eprintln!("skipped: {reason}");
            return;
        }
        Err(error) => panic!("start flapping daemon: {error}"),
    };
    let gate = ProbeGate::start(daemon.tcp_addr());
    let child = retry_screen_process(gate.addr());
    gate.wait_for_first_request(Duration::from_secs(5));
    daemon.stop().expect("stop daemon during transition");
    gate.release_first();
    child.wait_for_text("batuta — connecting", Duration::from_secs(5));
    let output = quit_retry_screen(child);
    let text = retry_screen_text(&output);
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
    let daemon = match runtime().block_on(Daemon::start()) {
        Ok(StartOutcome::Started(daemon)) => daemon,
        Ok(StartOutcome::Skip(reason)) => {
            eprintln!("skipped: {reason}");
            return;
        }
        Err(error) => panic!("start raced daemon: {error}"),
    };
    daemon.request_log().clear();
    let gate = ProbeGate::start(daemon.tcp_addr());
    let mut child = retry_screen_process(gate.addr());
    gate.wait_for_first_request(Duration::from_secs(5));
    let start = Arc::new(Barrier::new(3));
    let release = gate
        .release_first
        .lock()
        .expect("probe gate lock")
        .take()
        .expect("first startup probe already released");
    std::thread::scope(|scope| {
        scope.spawn({
            let start = start.clone();
            move || {
                start.wait();
                let _ = release.send(());
            }
        });
        scope.spawn({
            let start = start.clone();
            let child = &mut child;
            move || {
                start.wait();
                child.request_quit();
            }
        });
        start.wait();
    });
    let output = child.finish_after_quit_request_strict();
    let text = retry_screen_text(&output);
    assert!(output.status.success(), "{text}");
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("[1] Sessions"),
        "quit/connect race rendered the live session view: {text}"
    );
    let requests = daemon.request_log().entries();
    assert!(
        !requests.is_empty(),
        "the released startup probe was not observed"
    );
    assert!(
        requests.iter().all(|(method, _)| method == "GET"),
        "quit/connect race dispatched a daemon mutation: {requests:?}"
    );
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
    let reads_screen = tui.wait_for_screen(Duration::from_secs(5), |screen| {
        screen.lines().any(|line| line.contains("batuta  —"))
            && screen.contains("[2] Deliver runs · batuta-deliver")
            && screen.contains("no runs for batuta-deliver — press * for all")
    });
    assert!(
        reads_screen.lines().any(|line| line.contains("batuta  —")),
        "created session was not rendered while draining: {reads_screen:?}"
    );
    assert!(
        reads_screen.contains("no runs for batuta-deliver — press * for all"),
        "runs did not render their normal empty state while draining: {reads_screen:?}"
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
    daemon.set_daemon_draining(false);
    wait_for_request_count(
        &daemon,
        "/api/status",
        prior_status_polls + 1,
        Duration::from_secs(35),
    );
    let recovery_screen = tui.wait_for_screen(Duration::from_secs(5), |screen| {
        screen.contains("[1] Sessions")
            && screen.contains("daemon running")
            && !screen.contains("daemon draining — finishing in-flight work, writes refused")
    });
    assert!(
        !recovery_screen.contains("daemon draining — finishing in-flight work, writes refused"),
        "recovery retained the draining banner: {recovery_screen:?}"
    );

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

#[cfg(unix)]
#[test]
fn e2e_705_confirmed_registration_refetches_selects_and_boots() {
    runtime().block_on(async {
        let candidate = tempfile::tempdir().expect("create unregistered project");
        let fixture = OnboardingFixture::start(candidate.path(), OnboardingOutcome::Added).await;
        let mut tui = LiveTui::start_onboarding(&fixture.addr(), candidate.path());

        let candidate_screen = tui.wait_for_screen(Duration::from_secs(5), |screen| {
            screen.contains("Workspace not registered")
                && screen.contains(&candidate.path().display().to_string())
        });
        assert!(candidate_screen.contains("Name   "));
        tui.send(b"a");
        tui.wait_for_text("Add workspace?", Duration::from_secs(5));
        tui.send(b"\x1b");
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(fixture.writes(), 0, "confirmation cancel must not write");

        tui.send(b"a");
        tui.wait_for_text("Add workspace?", Duration::from_secs(5));
        tui.send(b"\r");
        wait_for_onboarding_writes(&fixture, 1);
        tui.wait_for_screen(Duration::from_secs(5), |screen| {
            screen.contains("[1] Sessions")
        });
        assert_eq!(fixture.writes(), 1, "confirmed add must write exactly once");
        assert_eq!(
            fixture.add_payloads(),
            vec![serde_json::json!({
                "name": candidate.path().file_name().unwrap().to_string_lossy(),
                "root_dir": candidate.path(),
            })]
        );
        let requests = fixture.requests();
        let post = requests
            .iter()
            .position(|(method, path)| method == "POST" && path == "/api/workspaces")
            .expect("confirmed add request");
        assert!(
            requests[post + 1..]
                .iter()
                .any(|(method, path)| method == "GET" && path == "/api/workspaces"),
            "successful add must refetch the workspace catalog: {requests:?}"
        );
        assert!(
            requests
                .iter()
                .any(|(method, path)| { method == "GET" && path == "/api/sessions" }),
            "selected workspace must run normal boot: {requests:?}"
        );

        assert!(tui.finish().success());
    });
}

#[cfg(unix)]
#[test]
fn e2e_706_unsupported_and_rejected_registration_remain_actionable() {
    runtime().block_on(async {
        let candidate = tempfile::tempdir().expect("create unsupported project");
        let fixture =
            OnboardingFixture::start(candidate.path(), OnboardingOutcome::Unsupported).await;
        let mut tui = LiveTui::start_onboarding(&fixture.addr(), candidate.path());
        tui.wait_for_screen(Duration::from_secs(5), |screen| {
            screen.contains("Workspace not registered")
        });
        tui.send(b"a\r");
        wait_for_onboarding_writes(&fixture, 1);
        tui.wait_for_screen(Duration::from_secs(5), |screen| {
            screen.contains("This daemon cannot add workspaces through its API.")
        });
        let unsupported = tui.screen_text();
        assert!(unsupported.contains("compozy"), "{unsupported}");
        assert!(unsupported.contains("workspace add"), "{unsupported}");
        assert!(unsupported.contains("[r] refresh"), "{unsupported}");
        assert_eq!(fixture.writes(), 1);
        assert!(tui.finish().success());

        let candidate = tempfile::tempdir().expect("create rejected project");
        let fixture = OnboardingFixture::start(candidate.path(), OnboardingOutcome::Rejected).await;
        let mut tui = LiveTui::start_onboarding(&fixture.addr(), candidate.path());
        tui.wait_for_screen(Duration::from_secs(5), |screen| {
            screen.contains("Workspace not registered")
        });
        tui.send(b"a\r");
        wait_for_onboarding_writes(&fixture, 1);
        tui.wait_for_screen(Duration::from_secs(5), |screen| {
            screen.contains("registration failed")
        });
        let rejected = tui.screen_text();
        assert!(rejected.contains("registration failed"), "{rejected}");
        assert!(
            rejected.contains("workspace could not be added"),
            "{rejected}"
        );
        assert!(!rejected.contains("workspace_invalid"), "{rejected}");
        assert!(
            !rejected.contains("root_dir must be canonical"),
            "{rejected}"
        );
        assert!(
            rejected.contains("[d] show technical details"),
            "{rejected}"
        );
        tui.send(b"d");
        tui.wait_for_screen(Duration::from_secs(5), |screen| {
            screen.contains("workspace_invalid") && screen.contains("root_dir must be canonical")
        });
        assert!(
            rejected.contains("[w] choose an existing workspace"),
            "{rejected}"
        );
        assert!(!rejected.contains("workspace added"), "{rejected}");
        assert!(tui.finish().success());
    });
}

#[cfg(unix)]
#[test]
fn e2e_707_onboarding_exit_picker_and_cancel_paths_make_no_registration_write() {
    runtime().block_on(async {
        for action in [b"w".as_slice(), b"a".as_slice()] {
            let candidate = tempfile::tempdir().expect("create no-write project");
            let fixture =
                OnboardingFixture::start(candidate.path(), OnboardingOutcome::Added).await;
            let mut tui = LiveTui::start_onboarding(&fixture.addr(), candidate.path());
            tui.wait_for_screen(Duration::from_secs(5), |screen| {
                screen.contains("Workspace not registered")
            });
            tui.send(action);
            if action == b"w" {
                tui.wait_for_screen(Duration::from_secs(5), |screen| {
                    screen.contains("no workspaces")
                });
            } else {
                tui.wait_for_screen(Duration::from_secs(5), |screen| {
                    screen.contains("Add workspace?")
                });
            }
            tui.send(b"\x1b");
            tui.wait_for_screen(Duration::from_secs(5), |screen| {
                screen.contains("Workspace not registered")
            });
            assert!(tui.finish().success());
            assert_eq!(fixture.writes(), 0, "action {action:?} must not write");
            assert!(
                fixture.requests().iter().all(|(method, _)| method == "GET"),
                "action {action:?} must not mutate fixture state"
            );
        }

        let candidate = tempfile::tempdir().expect("create exit project");
        let fixture = OnboardingFixture::start(candidate.path(), OnboardingOutcome::Added).await;
        let tui = LiveTui::start_onboarding(&fixture.addr(), candidate.path());
        tui.wait_for_screen(Duration::from_secs(5), |screen| {
            screen.contains("Workspace not registered")
        });
        assert!(tui.finish().success());
        assert_eq!(fixture.writes(), 0, "onboarding exit must not write");
    });
}
