use crate::{
    Client, Error, HttpClient,
    request::{RouteKind, response_error},
    types::{ErrorPayload, SessionStopped, TranscriptDelta, TranscriptSnapshot},
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamCursor {
    pub after_sequence: i64,
    pub epoch: i64,
    pub generation: i64,
}

#[derive(Clone, Debug)]
pub struct ReconnectPolicy {
    pub min: Duration,
    pub max: Duration,
    pub jitter: f32,
    pub idle_timeout: Duration,
    pub offline_after: u32,
    seed: Option<u64>,
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
    /// Makes reconnect jitter deterministic. Intended for tests and reproducible diagnostics.
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

pub struct ResetReason;

impl ResetReason {
    pub const FENCE_MISSING: &'static str = "fence_missing";
    pub const EPOCH_MISMATCH: &'static str = "epoch_mismatch";
    pub const GENERATION_MISMATCH: &'static str = "generation_mismatch";
    pub const SEQUENCE_RESET: &'static str = "sequence_reset";
}

#[derive(Debug)]
pub enum TranscriptEvent {
    Snapshot(TranscriptSnapshot),
    Delta(TranscriptDelta),
    Stopped(SessionStopped),
    ServerError(ErrorPayload),
    Lost {
        attempt: u32,
        next_in: Duration,
        error: String,
        offline_after: u32,
    },
    Reconnected,
    Fatal(Error),
}

enum MappedEvent {
    Emit(TranscriptEvent),
    Ignore,
    Stop(TranscriptEvent),
}

impl Client {
    /// Streams transcript events. The caller updates `cursor_source` after applying
    /// snapshots and deltas; the latest value is read before every connection attempt.
    pub fn transcript_stream(
        &self,
        workspace: &str,
        session_id: &str,
        mut cursor_source: watch::Receiver<StreamCursor>,
        policy: ReconnectPolicy,
    ) -> impl Stream<Item = TranscriptEvent> + Send + 'static {
        let client = self.clone();
        let workspace = workspace.to_owned();
        let session_id = session_id.to_owned();

        stream! {
            let mut cursor = *cursor_source.borrow_and_update();
            let mut source_cursor = cursor;
            let mut failures = 0_u32;
            let mut reconnecting = false;
            let seed = policy.seed.unwrap_or_else(rand::random);
            let mut rng = StdRng::seed_from_u64(seed);

            loop {
                let latest = *cursor_source.borrow_and_update();
                if latest != source_cursor {
                    cursor = latest;
                    source_cursor = latest;
                }

                tracing::debug!(
                    target: "sse.connect",
                    after_sequence = cursor.after_sequence,
                    epoch = cursor.epoch,
                    generation = cursor.generation,
                    attempt = failures
                );

                let response = match client
                    .connect_stream(&workspace, &session_id, cursor, policy.idle_timeout)
                    .await
                {
                    Ok(response) => response,
                    Err(ConnectFailure::Fatal(error)) => {
                        yield TranscriptEvent::Fatal(error);
                        break;
                    }
                    Err(ConnectFailure::Retry(error)) => {
                        failures = failures.saturating_add(1);
                        let next_in = policy.delay(failures, &mut rng);
                        trace_lost(failures, next_in, &error);
                        yield TranscriptEvent::Lost { attempt: failures, next_in, error, offline_after: policy.offline_after };
                        tokio::time::sleep(next_in).await;
                        reconnecting = true;
                        continue;
                    }
                };

                if reconnecting {
                    yield TranscriptEvent::Reconnected;
                }
                failures = 0;

                let (stopped, stream_error) = {
                    let body = response.into_body().into_data_stream();
                    let idle_timeout = policy.idle_timeout;
                    let byte_stream = stream! {
                        futures_util::pin_mut!(body);
                        loop {
                            match tokio::time::timeout(idle_timeout, body.next()).await {
                                Ok(Some(Ok(bytes))) => yield Ok::<Bytes, String>(bytes),
                                Ok(Some(Err(error))) => {
                                    yield Err(error.to_string());
                                    break;
                                }
                                Ok(None) => break,
                                Err(_) => {
                                    yield Err(format!("idle timeout after {idle_timeout:?}"));
                                    break;
                                }
                            }
                        }
                    };
                    let events = byte_stream.eventsource();
                    futures_util::pin_mut!(events);
                    let mut stopped = false;
                    let mut stream_error = None;

                    while let Some(result) = events.next().await {
                        match result {
                            Ok(event) => {
                                if !event.id.is_empty() {
                                    match event.id.parse::<i64>() {
                                        Ok(id) if id >= 0 => cursor.after_sequence = id,
                                        _ => {
                                            stream_error = Some(format!("invalid SSE event id: {}", event.id));
                                            break;
                                        }
                                    }
                                }
                                match map_event(&event.event, &event.id, &event.data) {
                                    Ok(MappedEvent::Emit(mapped)) => yield mapped,
                                    Ok(MappedEvent::Ignore) => {}
                                    Ok(MappedEvent::Stop(mapped)) => {
                                        yield mapped;
                                        stopped = true;
                                        break;
                                    }
                                    Err(error) => {
                                        stream_error = Some(error);
                                        break;
                                    }
                                }
                            }
                            Err(error) => {
                                stream_error = Some(error.to_string());
                                break;
                            }
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
                trace_lost(failures, next_in, &error);
                yield TranscriptEvent::Lost { attempt: failures, next_in, error, offline_after: policy.offline_after };
                tokio::time::sleep(next_in).await;
                reconnecting = true;
            }
        }
    }

    async fn connect_stream(
        &self,
        workspace: &str,
        session_id: &str,
        cursor: StreamCursor,
        idle_timeout: Duration,
    ) -> Result<hyper::Response<hyper::body::Incoming>, ConnectFailure> {
        let query = stream_query(cursor);
        let path = format!("/api/workspaces/{workspace}/sessions/{session_id}/stream?{query}");
        let uri = format!("{}{path}", self.base_uri)
            .parse::<http::Uri>()
            .map_err(|error| {
                ConnectFailure::Fatal(Error::Transport(format!("invalid URI: {error}")))
            })?;
        let mut builder = Request::builder().method(Method::GET).uri(uri);
        if matches!(self.transport, crate::Transport::Uds(_)) {
            builder = builder.header(HOST, "localhost");
        }
        if cursor.after_sequence > 0 {
            builder = builder.header("Last-Event-ID", cursor.after_sequence.to_string());
        }
        let request = builder
            .body(Empty::<Bytes>::new())
            .map_err(|error| ConnectFailure::Fatal(Error::Transport(error.to_string())))?;
        let operation = async {
            match &self.sse {
                HttpClient::Uds(client) => client.request(request).await,
                HttpClient::Tcp(client) => client.request(request).await,
            }
        };
        let response = operation
            .await
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
        let error = response_error(status, &body, &path, "transcript stream", RouteKind::Scoped);
        if status.is_client_error() {
            Err(ConnectFailure::Fatal(error))
        } else {
            Err(ConnectFailure::Retry(error.to_string()))
        }
    }
}

enum ConnectFailure {
    Retry(String),
    Fatal(Error),
}

fn stream_query(cursor: StreamCursor) -> String {
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("frames", "transcript");
    if cursor.epoch > 0 {
        serializer.append_pair("epoch", &cursor.epoch.to_string());
    }
    if cursor.generation > 0 {
        serializer.append_pair("generation", &cursor.generation.to_string());
    }
    if cursor.after_sequence > 0 {
        serializer.append_pair("after_sequence", &cursor.after_sequence.to_string());
    }
    serializer.finish()
}

fn map_event(event: &str, id: &str, data: &str) -> Result<MappedEvent, String> {
    let mapped = match event {
        "transcript_snapshot" => MappedEvent::Emit(TranscriptEvent::Snapshot(decode(
            data,
            "transcript_snapshot",
        )?)),
        "transcript_delta" => {
            MappedEvent::Emit(TranscriptEvent::Delta(decode(data, "transcript_delta")?))
        }
        "session_stopped" => {
            MappedEvent::Stop(TranscriptEvent::Stopped(decode(data, "session_stopped")?))
        }
        "error" => MappedEvent::Emit(TranscriptEvent::ServerError(decode(data, "error")?)),
        "goal_snapshot_changed" | "session_commands_changed" | "" => MappedEvent::Ignore,
        _ => MappedEvent::Ignore,
    };
    let (reset, reason, entries) = match &mapped {
        MappedEvent::Emit(TranscriptEvent::Snapshot(snapshot)) => (
            snapshot.reset,
            snapshot.reason.as_deref().unwrap_or(""),
            snapshot.entries.len(),
        ),
        MappedEvent::Emit(TranscriptEvent::Delta(delta)) => (false, "", delta.entries.len()),
        _ => (false, "", 0),
    };
    tracing::debug!(
        target: "sse.event",
        event,
        id,
        reset,
        reason,
        entries
    );
    Ok(mapped)
}

fn decode<T: serde::de::DeserializeOwned>(data: &str, context: &str) -> Result<T, String> {
    serde_json::from_str(data).map_err(|error| format!("decode {context}: {error}"))
}

fn trace_lost(attempt: u32, next_in: Duration, error: &str) {
    tracing::debug!(
        target: "sse.lost",
        error,
        attempt,
        next_in_ms = next_in.as_millis() as u64
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_omits_zero_fences() {
        assert_eq!(stream_query(StreamCursor::default()), "frames=transcript");
        assert_eq!(
            stream_query(StreamCursor {
                after_sequence: 3,
                epoch: 1,
                generation: 2
            }),
            "frames=transcript&epoch=1&generation=2&after_sequence=3"
        );
    }

    #[test]
    fn backoff_is_capped() {
        let policy = ReconnectPolicy {
            jitter: 0.0,
            ..ReconnectPolicy::default()
        };
        let mut rng = StdRng::seed_from_u64(1);
        let delays: Vec<_> = (1..=7)
            .map(|attempt| policy.delay(attempt, &mut rng))
            .collect();
        assert_eq!(
            delays,
            [500, 1_000, 2_000, 4_000, 8_000, 10_000, 10_000].map(Duration::from_millis)
        );
    }

    #[test]
    fn policy_and_reset_reason_defaults_match_the_contract() {
        let policy = ReconnectPolicy::default();
        assert_eq!(policy.min, Duration::from_millis(500));
        assert_eq!(policy.max, Duration::from_secs(10));
        assert_eq!(policy.jitter, 0.2);
        assert_eq!(policy.idle_timeout, Duration::from_secs(60));
        assert_eq!(policy.offline_after, 5);
        assert_eq!(ResetReason::FENCE_MISSING, "fence_missing");
        assert_eq!(ResetReason::EPOCH_MISMATCH, "epoch_mismatch");
        assert_eq!(ResetReason::GENERATION_MISMATCH, "generation_mismatch");
        assert_eq!(ResetReason::SEQUENCE_RESET, "sequence_reset");
    }
}
