use bytes::Bytes;
use futures_util::{Stream, stream};
use http::{Request, Response, StatusCode};
use http_body_util::{BodyExt, Full, StreamBody, combinators::UnsyncBoxBody};
use hyper::{
    body::{Frame, Incoming},
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use std::{
    collections::VecDeque,
    convert::Infallible,
    net::TcpListener as StdTcpListener,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{net::TcpStream, task::JoinHandle};

type TestBody = UnsyncBoxBody<Bytes, Infallible>;
type FrameStream = Pin<Box<dyn Stream<Item = Result<Frame<Bytes>, Infallible>> + Send>>;

#[derive(Clone, Debug)]
pub struct RecordedRequest {
    pub path_and_query: String,
    pub last_event_id: Option<String>,
}

#[derive(Clone)]
pub enum ResponseSpec {
    Sse(Vec<Bytes>),
    Timed(Vec<(std::time::Duration, Bytes)>),
    Status(u16, String),
    Hang,
    HangBeforeHeaders,
}

impl ResponseSpec {
    pub fn sse(body: impl Into<Bytes>) -> Self {
        Self::Sse(vec![body.into()])
    }

    pub fn chunks(chunks: Vec<Bytes>) -> Self {
        Self::Sse(chunks)
    }

    pub fn timed(chunks: Vec<(std::time::Duration, Bytes)>) -> Self {
        Self::Timed(chunks)
    }

    pub fn status(status: u16, body: impl Into<String>) -> Self {
        Self::Status(status, body.into())
    }
}

// Bounds each blocking accept() attempt so it always returns promptly: the
// accept loop alternates real (inhibiting) bursts with released gaps (see
// `SseServer::start`), and an unbounded blocking call would inhibit forever.
const ACCEPT_BURST: Duration = Duration::from_millis(5);
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_micros(100);

fn try_accept(
    listener: &StdTcpListener,
) -> std::io::Result<Option<(std::net::TcpStream, std::net::SocketAddr)>> {
    let deadline = std::time::Instant::now() + ACCEPT_BURST;
    loop {
        match listener.accept() {
            Ok(pair) => return Ok(Some(pair)),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if std::time::Instant::now() >= deadline {
                    return Ok(None);
                }
                std::thread::sleep(ACCEPT_POLL_INTERVAL);
            }
            Err(error) => return Err(error),
        }
    }
}

pub struct SseServer {
    addr: std::net::SocketAddr,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    active: Arc<AtomicUsize>,
    max_active: Arc<AtomicUsize>,
    closing: Arc<AtomicBool>,
    task: JoinHandle<()>,
}

impl SseServer {
    pub async fn start(responses: Vec<ResponseSpec>) -> Self {
        let listener = StdTcpListener::bind("127.0.0.1:0").expect("bind SSE server");
        listener
            .set_nonblocking(true)
            .expect("set SSE listener non-blocking");
        let addr = listener.local_addr().expect("SSE server address");
        let listener = Arc::new(listener);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let responses = Arc::new(Mutex::new(VecDeque::from(responses)));
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let closing = Arc::new(AtomicBool::new(false));

        let task_requests = Arc::clone(&requests);
        let task_active = Arc::clone(&active);
        let task_max_active = Arc::clone(&max_active);
        let task_closing = Arc::clone(&closing);
        let task = tokio::spawn(async move {
            loop {
                if task_closing.load(Ordering::SeqCst) {
                    break;
                }
                // Accept on the blocking pool rather than via the async listener.
                // Tests pair a paused (auto-advancing) client clock with a real
                // socket peer: tokio's auto-advance gives IO exactly one
                // non-blocking poll before fast-forwarding to the next timer, so
                // an async `accept().await` races the real (if brief) time the
                // handshake needs and loses deterministically. `spawn_blocking`
                // is tokio's documented mechanism to inhibit auto-advance for
                // its duration, which lets a real accept complete normally. The
                // accept itself is bounded (`try_accept`) and alternated with an
                // uninhibited sleep below so unrelated paused-clock timers (like
                // a client's reconnect backoff) still get to auto-advance
                // between bursts instead of freezing for the whole test.
                let accept_listener = Arc::clone(&listener);
                let accepted = tokio::task::spawn_blocking(move || try_accept(&accept_listener))
                    .await
                    .expect("accept task");
                let (socket, _) = match accepted {
                    Ok(Some(pair)) => pair,
                    Ok(None) => {
                        // Matches `ReconnectPolicy::min` (the shortest delay these
                        // tests ever wait on): the gap must divide evenly into every
                        // real timer paused-clock tests use, or auto-advance skips
                        // straight to *this* sleep instead of the timer a test is
                        // actually waiting on, and re-entering the burst right after
                        // realigns this loop with the moment a client wakes up to
                        // reconnect.
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        continue;
                    }
                    Err(_) => break,
                };
                if socket.set_nonblocking(true).is_err() {
                    break;
                }
                let Ok(socket) = TcpStream::from_std(socket) else {
                    break;
                };
                let requests = Arc::clone(&task_requests);
                let responses = Arc::clone(&responses);
                let active = Arc::clone(&task_active);
                let max_active = Arc::clone(&task_max_active);
                tokio::spawn(async move {
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_active.fetch_max(now, Ordering::SeqCst);
                    let service = service_fn(move |request: Request<Incoming>| {
                        let requests = Arc::clone(&requests);
                        let responses = Arc::clone(&responses);
                        async move {
                            requests
                                .lock()
                                .expect("request lock")
                                .push(RecordedRequest {
                                    path_and_query: request.uri().path_and_query().map_or_else(
                                        || request.uri().path().to_owned(),
                                        ToString::to_string,
                                    ),
                                    last_event_id: request
                                        .headers()
                                        .get("Last-Event-ID")
                                        .and_then(|value| value.to_str().ok())
                                        .map(str::to_owned),
                                });
                            let spec = responses
                                .lock()
                                .expect("response lock")
                                .pop_front()
                                .unwrap_or_else(|| {
                                    ResponseSpec::status(500, r#"{"error":"no scripted response"}"#)
                                });
                            if matches!(spec, ResponseSpec::HangBeforeHeaders) {
                                // Accepts the connection but never writes status line or
                                // headers, simulating a peer wedged before responding.
                                std::future::pending::<Result<Response<TestBody>, Infallible>>()
                                    .await
                            } else {
                                Ok::<_, Infallible>(response(spec))
                            }
                        }
                    });
                    let mut builder = http1::Builder::new();
                    builder.keep_alive(false);
                    let _ = builder
                        .serve_connection(TokioIo::new(socket), service)
                        .await;
                    active.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });
        Self {
            addr,
            requests,
            active,
            max_active,
            closing,
            task,
        }
    }

    pub fn address(&self) -> String {
        self.addr.to_string()
    }

    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().expect("request lock").clone()
    }

    pub fn active(&self) -> usize {
        self.active.load(Ordering::SeqCst)
    }

    pub fn max_active(&self) -> usize {
        self.max_active.load(Ordering::SeqCst)
    }
}

impl Drop for SseServer {
    fn drop(&mut self) {
        self.closing.store(true, Ordering::SeqCst);
        self.task.abort();
    }
}

fn response(spec: ResponseSpec) -> Response<TestBody> {
    match spec {
        ResponseSpec::Sse(chunks) => {
            let frames = chunks.into_iter().map(|bytes| Ok(Frame::data(bytes)));
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/event-stream")
                .body(StreamBody::new(Box::pin(stream::iter(frames)) as FrameStream).boxed_unsync())
                .expect("SSE response")
        }
        ResponseSpec::Timed(chunks) => {
            let frames = async_stream::stream! {
                for (delay, bytes) in chunks {
                    tokio::time::sleep(delay).await;
                    yield Ok(Frame::data(bytes));
                }
            };
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/event-stream")
                .body(StreamBody::new(Box::pin(frames) as FrameStream).boxed_unsync())
                .expect("timed SSE response")
        }
        ResponseSpec::Status(status, body) => Response::builder()
            .status(StatusCode::from_u16(status).expect("test status"))
            .body(Full::new(Bytes::from(body)).boxed_unsync())
            .expect("status response"),
        ResponseSpec::Hang => Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/event-stream")
            .body(StreamBody::new(Box::pin(stream::pending()) as FrameStream).boxed_unsync())
            .expect("hanging response"),
        ResponseSpec::HangBeforeHeaders => {
            unreachable!("HangBeforeHeaders is intercepted before building a response")
        }
    }
}
