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
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::{net::TcpListener, task::JoinHandle};

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

pub struct SseServer {
    addr: std::net::SocketAddr,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    active: Arc<AtomicUsize>,
    max_active: Arc<AtomicUsize>,
    task: JoinHandle<()>,
}

impl SseServer {
    pub async fn start(responses: Vec<ResponseSpec>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind SSE server");
        let addr = listener.local_addr().expect("SSE server address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let responses = Arc::new(Mutex::new(VecDeque::from(responses)));
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let task_requests = Arc::clone(&requests);
        let task_active = Arc::clone(&active);
        let task_max_active = Arc::clone(&max_active);
        let task = tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
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
                            Ok::<_, Infallible>(response(spec))
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
        tokio::task::yield_now().await;
        Self {
            addr,
            requests,
            active,
            max_active,
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
    }
}
