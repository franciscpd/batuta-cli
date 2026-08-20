use super::{Cursor, NoCursor};
use crate::{
    Client, Error, HttpClient,
    request::{RouteKind, response_error},
    types::ErrorPayload,
};
use async_stream::stream;
use bytes::Bytes;
use eventsource_stream::Eventsource;
use futures_util::{Stream, StreamExt};
use http::{Method, Request, header::HOST};
use http_body_util::{BodyExt, Empty};
use rand::{Rng, SeedableRng, rngs::StdRng};
use std::time::Duration;
use tokio::sync::watch;

#[derive(Clone, Debug)]
pub struct Frame {
    pub event: String,
    pub id: String,
    pub data: String,
}

#[derive(Clone, Debug)]
pub struct StreamRequest {
    pub path: String,
    pub context: &'static str,
    pub query: Vec<(&'static str, String)>,
    pub stop_event: Option<&'static str>,
    pub retry_server_errors: bool,
}

impl StreamRequest {
    pub fn new(path: impl Into<String>, context: &'static str) -> Self {
        Self {
            path: path.into(),
            context,
            query: Vec::new(),
            stop_event: None,
            retry_server_errors: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReconnectPolicy {
    pub min: Duration,
    pub max: Duration,
    pub jitter: f32,
    pub idle_timeout: Duration,
    pub offline_after: u32,
    pub(crate) seed: Option<u64>,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            min: Duration::from_millis(500),
            max: Duration::from_secs(10),
            jitter: 0.2,
            idle_timeout: Duration::from_secs(60),
            offline_after: 5,
            seed: None,
        }
    }
}

impl ReconnectPolicy {
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    fn delay(&self, attempt: u32, rng: &mut StdRng) -> Duration {
        let exponent = attempt.saturating_sub(1).min(63);
        let base = self
            .min
            .mul_f64(2_u64.saturating_pow(exponent).min(u32::MAX as u64) as f64)
            .min(self.max);
        let jitter = self.jitter.clamp(0.0, 1.0) as f64;
        if jitter == 0.0 {
            return base;
        }
        base.mul_f64(1.0 + rng.random_range(-jitter..=jitter))
    }
}

#[derive(Debug)]
pub enum StreamEvent<E> {
    Event(E),
    Lost {
        attempt: u32,
        next_in: Duration,
        error: String,
        offline_after: u32,
    },
    Reconnected,
    ServerError(ErrorPayload),
    Fatal(Error),
}

pub struct EventStream;

impl EventStream {
    pub fn open<C, E>(
        client: Client,
        request: StreamRequest,
        mut cursor_source: watch::Receiver<C>,
        policy: ReconnectPolicy,
        map: fn(&Frame) -> Option<E>,
    ) -> impl Stream<Item = StreamEvent<E>> + Send + 'static
    where
        C: Cursor,
        E: Send + 'static,
    {
        stream! {
            let mut cursor = cursor_source.borrow_and_update().clone();
            let mut source_cursor = cursor.clone();
            let mut failures = 0_u32;
            let mut reconnecting = false;
            let mut rng = StdRng::seed_from_u64(policy.seed.unwrap_or_else(rand::random));

            loop {
                let latest = cursor_source.borrow_and_update().clone();
                if latest.last_event_id() != source_cursor.last_event_id() || latest.query() != source_cursor.query() {
                    cursor = latest.clone();
                    source_cursor = latest;
                }
                let response = match connect(&client, &request, &cursor, policy.idle_timeout).await {
                    Ok(response) => response,
                    Err(ConnectFailure::Fatal(error)) => {
                        yield StreamEvent::Fatal(error);
                        break;
                    }
                    Err(ConnectFailure::Retry(error)) => {
                        failures = failures.saturating_add(1);
                        let next_in = policy.delay(failures, &mut rng);
                        yield StreamEvent::Lost { attempt: failures, next_in, error, offline_after: policy.offline_after };
                        tokio::time::sleep(next_in).await;
                        reconnecting = true;
                        continue;
                    }
                };

                if reconnecting {
                    yield StreamEvent::Reconnected;
                }
                failures = 0;

                let (stopped, stream_error) = {
                    let body = response.into_body().into_data_stream();
                    let idle_timeout = policy.idle_timeout;
                    let bytes = stream! {
                        futures_util::pin_mut!(body);
                        loop {
                            match tokio::time::timeout(idle_timeout, body.next()).await {
                                Ok(Some(Ok(bytes))) => yield Ok::<Bytes, String>(bytes),
                                Ok(Some(Err(error))) => { yield Err(error.to_string()); break; }
                                Ok(None) => break,
                                Err(_) => { yield Err(format!("idle timeout after {idle_timeout:?}")); break; }
                            }
                        }
                    };
                    let events = bytes.eventsource();
                    futures_util::pin_mut!(events);
                    let mut stopped = false;
                    let mut stream_error = None;
                    while let Some(result) = events.next().await {
                        match result {
                            Ok(event) => {
                                if !event.id.is_empty() {
                                    cursor.apply_id(&event.id);
                                }
                                let frame = Frame { event: event.event, id: event.id, data: event.data };
                                if frame.event == "error" {
                                    match serde_json::from_str(&frame.data) {
                                        Ok(payload) => yield StreamEvent::ServerError(payload),
                                        Err(error) => { stream_error = Some(format!("decode error: {error}")); break; }
                                    }
                                } else if let Some(mapped) = map(&frame) {
                                    yield StreamEvent::Event(mapped);
                                }
                                if request.stop_event == Some(frame.event.as_str()) {
                                    stopped = true;
                                    break;
                                }
                            }
                            Err(error) => { stream_error = Some(error.to_string()); break; }
                        }
                    }
                    (stopped, stream_error)
                };
                if stopped {
                    break;
                }
                failures = failures.saturating_add(1);
                let error = stream_error.unwrap_or_else(|| "stream ended".to_owned());
                let next_in = policy.delay(failures, &mut rng);
                yield StreamEvent::Lost { attempt: failures, next_in, error, offline_after: policy.offline_after };
                tokio::time::sleep(next_in).await;
                reconnecting = true;
            }
        }
    }
}

pub(crate) async fn handshake(client: &Client, request: &StreamRequest) -> Result<(), Error> {
    match connect(client, request, &NoCursor, client.request_timeout).await {
        Ok(_) => Ok(()),
        Err(ConnectFailure::Fatal(error)) => Err(error),
        Err(ConnectFailure::Retry(error)) => Err(Error::Transport(error)),
    }
}

enum ConnectFailure {
    Retry(String),
    Fatal(Error),
}

async fn connect<C: Cursor>(
    client: &Client,
    request: &StreamRequest,
    cursor: &C,
    idle_timeout: Duration,
) -> Result<hyper::Response<hyper::body::Incoming>, ConnectFailure> {
    let query = encode_query(request, cursor);
    let path = if query.is_empty() {
        request.path.clone()
    } else {
        format!("{}?{}", request.path, query)
    };
    let uri = format!("{}{path}", client.base_uri)
        .parse::<http::Uri>()
        .map_err(|error| {
            ConnectFailure::Fatal(Error::Transport(format!("invalid URI: {error}")))
        })?;
    let mut builder = Request::builder().method(Method::GET).uri(uri);
    if matches!(client.transport, crate::Transport::Uds(_)) {
        builder = builder.header(HOST, "localhost");
    }
    if let Some(id) = cursor.last_event_id() {
        builder = builder.header("Last-Event-ID", id);
    }
    let request_message = builder
        .body(Empty::<Bytes>::new().boxed())
        .map_err(|error| ConnectFailure::Fatal(Error::Transport(error.to_string())))?;
    let operation = async {
        match &client.sse {
            HttpClient::Uds(client) => client.request(request_message).await,
            HttpClient::Tcp(client) => client.request(request_message).await,
        }
    };
    let response = tokio::time::timeout(idle_timeout, operation)
        .await
        .map_err(|_| ConnectFailure::Retry(format!("idle timeout after {idle_timeout:?}")))?
        .map_err(|error| ConnectFailure::Retry(error.to_string()))?;
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = tokio::time::timeout(idle_timeout, response.into_body().collect())
        .await
        .map_err(|_| ConnectFailure::Retry(format!("idle timeout after {idle_timeout:?}")))?
        .map_err(|error| ConnectFailure::Retry(error.to_string()))?
        .to_bytes();
    let error = response_error(
        status,
        &body,
        "GET",
        &path,
        request.context,
        RouteKind::Scoped,
    );
    if status.is_client_error() || !request.retry_server_errors {
        Err(ConnectFailure::Fatal(error))
    } else {
        Err(ConnectFailure::Retry(error.to_string()))
    }
}

fn encode_query<C: Cursor>(request: &StreamRequest, cursor: &C) -> String {
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    for (name, value) in request.query.iter().cloned().chain(cursor.query()) {
        serializer.append_pair(name, &value);
    }
    serializer.finish()
}
