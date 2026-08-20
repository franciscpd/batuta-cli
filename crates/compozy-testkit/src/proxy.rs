use crate::{Error, Result};
use std::{
    net::SocketAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinHandle,
};

#[derive(Clone, Default)]
pub struct RequestLog(Arc<Mutex<Vec<(String, String)>>>);

impl RequestLog {
    pub fn entries(&self) -> Vec<(String, String)> {
        self.0.lock().expect("request log lock poisoned").clone()
    }

    pub fn clear(&self) {
        self.0.lock().expect("request log lock poisoned").clear();
    }

    fn push(&self, method: &str, path: &str) {
        self.0
            .lock()
            .expect("request log lock poisoned")
            .push((method.to_owned(), path.to_owned()));
    }
}

#[derive(Clone, Default)]
pub(crate) struct Faults {
    catalog_draining: Arc<AtomicBool>,
    daemon_draining: Arc<AtomicBool>,
    prompt_delay_ms: Arc<AtomicU64>,
    catalog_connections: Arc<AtomicUsize>,
}

impl Faults {
    pub(crate) fn set_catalog_draining(&self, draining: bool) {
        self.catalog_draining.store(draining, Ordering::SeqCst);
    }

    pub(crate) fn set_daemon_draining(&self, draining: bool) {
        self.daemon_draining.store(draining, Ordering::SeqCst);
    }

    pub(crate) fn set_prompt_delay(&self, delay: Duration) {
        self.prompt_delay_ms.store(
            u64::try_from(delay.as_millis()).unwrap_or(u64::MAX),
            Ordering::SeqCst,
        );
    }

    pub(crate) fn active_catalog_connections(&self) -> usize {
        self.catalog_connections.load(Ordering::SeqCst)
    }

    fn catalog_draining(&self) -> bool {
        self.catalog_draining.load(Ordering::SeqCst)
    }

    fn daemon_draining(&self) -> bool {
        self.daemon_draining.load(Ordering::SeqCst)
    }

    fn prompt_delay(&self) -> Duration {
        Duration::from_millis(self.prompt_delay_ms.load(Ordering::SeqCst))
    }
}

struct CatalogConnection(Arc<AtomicUsize>);

impl CatalogConnection {
    fn start(faults: &Faults) -> Self {
        faults.catalog_connections.fetch_add(1, Ordering::SeqCst);
        Self(faults.catalog_connections.clone())
    }
}

impl Drop for CatalogConnection {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

pub(crate) struct Proxy {
    addr: SocketAddr,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl Proxy {
    pub async fn start(target: SocketAddr, log: RequestLog, faults: Faults) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let addr = listener.local_addr()?;
        let (stop, mut stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut stopped => break,
                    accepted = listener.accept() => {
                        let Ok((client, _)) = accepted else { break };
                        let log = log.clone();
                        let faults = faults.clone();
                        tokio::spawn(async move {
                            let _ = forward(client, target, log, faults).await;
                        });
                    }
                }
            }
        });
        Ok(Self {
            addr,
            stop: Some(stop),
            task,
        })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.task.abort();
    }
}

async fn forward(
    mut client: TcpStream,
    target: SocketAddr,
    log: RequestLog,
    faults: Faults,
) -> Result<()> {
    let mut request = Vec::with_capacity(4096);
    let header_end = loop {
        if request.len() > 1024 * 1024 {
            return Err(Error::Message("proxy request headers exceed 1 MiB".into()));
        }
        let mut chunk = [0_u8; 4096];
        let read = client.read(&mut chunk).await?;
        if read == 0 {
            return Ok(());
        }
        request.extend_from_slice(&chunk[..read]);
        if let Some(index) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let headers = String::from_utf8_lossy(&request[..header_end]).into_owned();
    let first = headers.lines().next().unwrap_or_default();
    let mut fields = first.split_whitespace();
    let method = fields.next().unwrap_or_default();
    let path = fields.next().unwrap_or_default();
    log.push(method, path);
    if method == "GET" && path == "/api/status" && faults.daemon_draining() {
        write_draining(&mut client).await?;
        return Ok(());
    }
    if method == "GET" && path == "/api/sessions/catalog-stream" && faults.catalog_draining() {
        write_draining(&mut client).await?;
        return Ok(());
    }
    let content_length = headers
        .lines()
        .find_map(|line| {
            line.strip_prefix("Content-Length:")
                .or_else(|| line.strip_prefix("content-length:"))
        })
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    while request.len() < header_end + content_length {
        let mut chunk = [0_u8; 4096];
        let read = client.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..read]);
    }
    if method == "POST" && path.ends_with("/prompt") {
        tokio::time::sleep(faults.prompt_delay()).await;
    }
    if !matches!(method, "GET" | "HEAD") && faults.daemon_draining() {
        write_draining(&mut client).await?;
        return Ok(());
    }

    let mut forwarded = Vec::with_capacity(request.len() + 24);
    for (index, line) in headers.trim_end_matches("\r\n").split("\r\n").enumerate() {
        if index > 0 && line.to_ascii_lowercase().starts_with("connection:") {
            continue;
        }
        forwarded.extend_from_slice(line.as_bytes());
        forwarded.extend_from_slice(b"\r\n");
    }
    forwarded.extend_from_slice(b"Connection: close\r\n\r\n");
    forwarded.extend_from_slice(&request[header_end..]);

    let _catalog_connection = (method == "GET" && path == "/api/sessions/catalog-stream")
        .then(|| CatalogConnection::start(&faults));
    let mut upstream = TcpStream::connect(target).await?;
    upstream.write_all(&forwarded).await?;
    let _ = tokio::io::copy_bidirectional(&mut upstream, &mut client).await?;
    Ok(())
}

async fn write_draining(client: &mut TcpStream) -> Result<()> {
    let body = r#"{"error":"daemon is draining"}"#;
    let response = format!(
        "HTTP/1.1 503 Service Unavailable\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    client.write_all(response.as_bytes()).await?;
    Ok(())
}
