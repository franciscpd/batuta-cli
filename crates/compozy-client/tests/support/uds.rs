use crate::support::fixture;
use bytes::Bytes;
use http::{Request, Response, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper::{body::Incoming, server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use std::{
    collections::VecDeque,
    convert::Infallible,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{net::UnixListener, task::JoinHandle};

static NEXT_SOCKET: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
pub struct RecordedRequest {
    pub method: String,
    pub path_and_query: String,
    pub host: Option<String>,
    pub body: Bytes,
}

#[derive(Clone)]
pub struct ResponseSpec {
    status: StatusCode,
    body: String,
    delay: Option<Duration>,
    hang: bool,
}

impl ResponseSpec {
    pub fn json(status: u16, body: impl Into<String>) -> Self {
        Self {
            status: StatusCode::from_u16(status).expect("valid test status"),
            body: body.into(),
            delay: None,
            hang: false,
        }
    }

    pub fn delayed(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    pub fn hanging() -> Self {
        Self {
            status: StatusCode::OK,
            body: String::new(),
            delay: None,
            hang: true,
        }
    }
}

pub struct UdsServer {
    pub path: PathBuf,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    task: JoinHandle<()>,
}

impl UdsServer {
    pub async fn start(responses: Vec<ResponseSpec>) -> Self {
        let path = std::env::temp_dir().join(format!(
            "compozy-client-{}-{}.sock",
            std::process::id(),
            NEXT_SOCKET.fetch_add(1, Ordering::Relaxed)
        ));
        let listener = UnixListener::bind(&path).expect("bind test UDS");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let queue = Arc::new(Mutex::new(VecDeque::from(responses)));
        let task_requests = Arc::clone(&requests);
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let connection_requests = Arc::clone(&task_requests);
                let connection_queue = Arc::clone(&queue);
                tokio::spawn(async move {
                    let service = service_fn(move |request: Request<Incoming>| {
                        let requests = Arc::clone(&connection_requests);
                        let queue = Arc::clone(&connection_queue);
                        async move {
                            let (parts, body) = request.into_parts();
                            let body = body
                                .collect()
                                .await
                                .expect("collect test request body")
                                .to_bytes();
                            requests
                                .lock()
                                .expect("requests lock")
                                .push(RecordedRequest {
                                    method: parts.method.to_string(),
                                    path_and_query: parts.uri.path_and_query().map_or_else(
                                        || parts.uri.path().to_owned(),
                                        ToString::to_string,
                                    ),
                                    host: parts
                                        .headers
                                        .get(http::header::HOST)
                                        .and_then(|value| value.to_str().ok())
                                        .map(str::to_owned),
                                    body,
                                });
                            let response = queue
                                .lock()
                                .expect("response lock")
                                .pop_front()
                                .unwrap_or_else(|| {
                                    ResponseSpec::json(500, r#"{"error":"no response"}"#)
                                });
                            if response.hang {
                                std::future::pending::<()>().await;
                            }
                            if let Some(delay) = response.delay {
                                tokio::time::sleep(delay).await;
                            }
                            Ok::<_, Infallible>(
                                Response::builder()
                                    .status(response.status)
                                    .body(Full::new(Bytes::from(response.body)))
                                    .expect("test response"),
                            )
                        }
                    });
                    let _ = http1::Builder::new()
                        .serve_connection(TokioIo::new(stream), service)
                        .await;
                });
            }
        });
        Self {
            path,
            requests,
            task,
        }
    }

    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().expect("requests lock").clone()
    }
}

impl Drop for UdsServer {
    fn drop(&mut self) {
        self.task.abort();
        let _ = std::fs::remove_file(&self.path);
    }
}

pub fn status_response() -> ResponseSpec {
    ResponseSpec::json(200, fixture("status.json"))
}
