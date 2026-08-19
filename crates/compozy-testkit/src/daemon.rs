use crate::{
    Error, Result, config_seed, env,
    proxy::{Faults, Proxy, RequestLog},
    readiness,
};
use bytes::Bytes;
use compozy_client::Client;
use http::{Method, Request};
use http_body_util::{BodyExt, Full};
use hyper_util::{
    client::legacy::{Client as HyperClient, connect::HttpConnector},
    rt::TokioExecutor,
};
use std::{
    fs::{self, OpenOptions},
    net::SocketAddr,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use tempfile::TempDir;

pub enum StartOutcome {
    Started(Box<Daemon>),
    Skip(&'static str),
}

#[derive(Clone, Debug)]
pub struct DaemonOptions {
    binary: Option<PathBuf>,
    search_path: bool,
    http_port: Option<u16>,
    readiness_timeout: Duration,
}

impl Default for DaemonOptions {
    fn default() -> Self {
        Self {
            binary: None,
            search_path: true,
            http_port: None,
            readiness_timeout: Duration::from_secs(30),
        }
    }
}

impl DaemonOptions {
    pub fn binary(mut self, path: impl Into<PathBuf>) -> Self {
        self.binary = Some(path.into());
        self
    }

    pub fn without_path_lookup(mut self) -> Self {
        self.search_path = false;
        self
    }

    pub fn http_port(mut self, port: u16) -> Self {
        self.http_port = Some(port);
        self
    }

    pub fn readiness_timeout(mut self, timeout: Duration) -> Self {
        self.readiness_timeout = timeout;
        self
    }
}

pub struct Daemon {
    binary: PathBuf,
    home: Option<TempDir>,
    socket: PathBuf,
    daemon_addr: SocketAddr,
    workspace_id: String,
    process: Option<Child>,
    proxy: Proxy,
    requests: RequestLog,
    faults: Faults,
    start_attempts: usize,
}

impl Daemon {
    pub async fn start() -> Result<StartOutcome> {
        Self::start_with(DaemonOptions::default()).await
    }

    pub async fn start_with(options: DaemonOptions) -> Result<StartOutcome> {
        let Some(binary) = env::discover(options.binary.as_deref(), options.search_path)? else {
            return Ok(StartOutcome::Skip(env::SKIP_REASON));
        };
        ensure_checkout_is_safe()?;
        let home = tempfile::Builder::new()
            .prefix("batuta-ct-home-")
            .tempdir()?;
        ensure_child_cwd_is_safe(home.path())?;
        let socket = config_seed::short_socket_path();
        let log_path = home.path().join("daemon-process.log");
        let path = std::env::var_os("PATH");
        let mut port = match options.http_port {
            Some(port) => port,
            None => config_seed::free_port()?,
        };
        let mut attempts = 0;
        let process = loop {
            attempts += 1;
            config_seed::write(home.path(), port, &socket)?;
            let mut child = launch(&binary, home.path(), &log_path, path.as_deref())?;
            match readiness::wait(&mut child, &socket, &log_path, options.readiness_timeout).await {
                Ok(()) => break child,
                Err(failure) if failure.port_collision && attempts == 1 => {
                    terminate_group(&mut child);
                    let _ = fs::remove_file(&socket);
                    port = config_seed::free_port()?;
                }
                Err(failure) => {
                    terminate_group(&mut child);
                    let _ = fs::remove_file(&socket);
                    return Err(Error::Message(failure.message));
                }
            }
        };

        let daemon_addr = SocketAddr::from(([127, 0, 0, 1], port));
        let requests = RequestLog::default();
        let faults = Faults::default();
        let proxy = Proxy::start(daemon_addr, requests.clone(), faults.clone()).await?;
        let workspaces = Client::uds(socket.clone())
            .workspaces()
            .await
            .map_err(|error| Error::Message(format!("discover default workspace: {error}")))?;
        let workspace_id = workspaces
            .first()
            .map(|workspace| workspace.id.clone())
            .filter(|id| !id.is_empty())
            .ok_or_else(|| Error::Message("daemon registered no default workspace".into()))?;

        Ok(StartOutcome::Started(Box::new(Self {
            binary,
            home: Some(home),
            socket,
            daemon_addr,
            workspace_id,
            process: Some(process),
            proxy,
            requests,
            faults,
            start_attempts: attempts,
        })))
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket
    }

    pub fn tcp_addr(&self) -> SocketAddr {
        self.proxy.addr()
    }

    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    pub fn request_log(&self) -> RequestLog {
        self.requests.clone()
    }

    pub fn set_catalog_draining(&self, draining: bool) {
        self.faults.set_catalog_draining(draining);
    }

    pub fn home_path(&self) -> &Path {
        self.home
            .as_ref()
            .expect("daemon home already removed")
            .path()
    }

    pub fn process_id(&self) -> u32 {
        self.process.as_ref().expect("daemon process missing").id()
    }

    pub fn start_attempts(&self) -> usize {
        self.start_attempts
    }

    pub async fn create_session(&self, agent_name: &str) -> Result<String> {
        let payload = serde_json::to_vec(&serde_json::json!({
            "workspace": self.workspace_id,
            "agent_name": agent_name,
        }))?;
        let request = Request::builder()
            .method(Method::POST)
            .uri(format!("http://{}/api/sessions", self.tcp_addr()))
            .header("content-type", "application/json")
            .body(Full::new(Bytes::from(payload)))
            .map_err(|error| Error::Message(format!("build create-session request: {error}")))?;
        let connector = HttpConnector::new();
        let client = HyperClient::builder(TokioExecutor::new()).build(connector);
        let response = client
            .request(request)
            .await
            .map_err(|error| Error::Message(format!("create session request: {error}")))?;
        let status = response.status();
        let body = response
            .into_body()
            .collect()
            .await
            .map_err(|error| Error::Message(format!("read create-session response: {error}")))?
            .to_bytes();
        if !status.is_success() {
            return Err(Error::Message(format!(
                "create session returned HTTP {status}: {}",
                String::from_utf8_lossy(&body)
            )));
        }
        let value: serde_json::Value = serde_json::from_slice(&body)?;
        value
            .pointer("/session/id")
            .or_else(|| value.get("id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| Error::Message(format!("create session response has no id: {value}")))
    }

    pub fn daemon_tcp_addr(&self) -> SocketAddr {
        self.daemon_addr
    }

    pub fn stop(mut self) -> Result<()> {
        self.shutdown(true)
    }

    fn shutdown(&mut self, require_clean_stop: bool) -> Result<()> {
        let Some(home) = self.home.as_ref().map(|home| home.path().to_path_buf()) else {
            return Ok(());
        };
        let path = std::env::var_os("PATH");
        let mut stop = Command::new(&self.binary);
        stop.args(["daemon", "stop", "-o", "json"]);
        env::configure(&mut stop, &home, path.as_deref());
        stop.stdout(Stdio::null()).stderr(Stdio::null());
        let clean_stop = if let Ok(mut child) = stop.spawn() {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut status = None;
            while Instant::now() < deadline {
                match child.try_wait() {
                    Ok(Some(done)) => {
                        status = Some(done);
                        break;
                    }
                    Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                    Err(_) => break,
                }
            }
            if status.is_none() {
                let _ = child.kill();
                let _ = child.wait();
            }
            status.is_some_and(|status| status.success())
        } else {
            false
        };
        if let Some(mut process) = self.process.take() {
            terminate_group(&mut process);
        }
        let _ = fs::remove_file(&self.socket);
        self.home.take();
        if require_clean_stop && !clean_stop {
            return Err(Error::Message(
                "compozy daemon stop -o json did not finish successfully".into(),
            ));
        }
        Ok(())
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.shutdown(false);
    }
}

fn launch(
    binary: &Path,
    home: &Path,
    log_path: &Path,
    path: Option<&std::ffi::OsStr>,
) -> Result<Child> {
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    let stderr = log.try_clone()?;
    let mut command = Command::new(binary);
    command.args(["daemon", "start", "--foreground", "--exit-when-orphaned"]);
    env::configure(&mut command, home, path);
    command
        .process_group(0)
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr));
    command.spawn().map_err(Error::from)
}

fn terminate_group(child: &mut Child) {
    let pid = child.id() as i32;
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn ensure_checkout_is_safe() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let checkout = cwd
        .ancestors()
        .find(|candidate| candidate.join(".git").exists())
        .unwrap_or(&cwd);
    if checkout.join(".compozy").exists() {
        return Err(Error::Message(format!(
            "refusing to run contract daemon from {} because it contains .compozy; use a disposable detached worktree",
            checkout.display()
        )));
    }
    Ok(())
}

fn ensure_child_cwd_is_safe(path: &Path) -> Result<()> {
    if path.join(".compozy").exists() {
        return Err(Error::Message(format!(
            "refusing unsafe harness cwd: {}",
            path.display()
        )));
    }
    Ok(())
}
