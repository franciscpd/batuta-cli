#[path = "support/sse_server.rs"]
mod sse_server;
mod support;

use bytes::Bytes;
use compozy_client::{Client, Error, ReconnectPolicy, StreamCursor, TranscriptEvent};
use futures_util::{StreamExt, pin_mut};
use sse_server::{ResponseSpec, SseServer};
use std::time::Duration;
use tokio::sync::watch;

fn channel(cursor: StreamCursor) -> (watch::Sender<StreamCursor>, watch::Receiver<StreamCursor>) {
    watch::channel(cursor)
}

fn exact_policy() -> ReconnectPolicy {
    let mut policy = ReconnectPolicy::default();
    policy.jitter = 0.0;
    policy.with_seed(7)
}

fn snapshot(id: i64, reset: bool, reason: Option<&str>) -> String {
    let reason = reason.map_or(String::new(), |value| format!(",\"reason\":\"{value}\""));
    format!(
        "id: {id}\nevent: transcript_snapshot\ndata: {{\"session_id\":\"sess\",\"epoch\":1,\"generation\":2,\"entries\":[],\"max_sequence\":{id},\"reset\":{reset}{reason}}}\n\n"
    )
}

fn delta(id: i64) -> String {
    format!(
        "id: {id}\nevent: transcript_delta\ndata: {{\"epoch\":1,\"generation\":2,\"entries\":[],\"cursor\":{id},\"max_sequence\":{id},\"has_more\":false}}\n\n"
    )
}

async fn wait_for_requests(server: &SseServer, count: usize) {
    // `SseServer` accepts in bounded (real-time) bursts rather than a single
    // instant async accept (see its module docs), so a request can take a few
    // milliseconds of wall-clock time to be recorded even on a fresh
    // connection; poll with a small real sleep rather than a bare
    // `yield_now`, which can exhaust its budget in well under a millisecond.
    for _ in 0..200 {
        if server.requests().len() >= count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("expected {count} requests, got {}", server.requests().len());
}

#[tokio::test]
async fn ut_040_split_fixture_parses_snapshot_and_deltas() {
    let fixture = format!(
        "{}data: {{\"epoch\":0,\"generation\":0,\"entries\":[],\"cursor\":390,\"max_sequence\":390,\"has_more\":false}}\n\n",
        support::fixture("stream_normal.sse")
    );
    for split in [1, fixture.len() / 2, fixture.len() - 1] {
        let chunks = vec![
            Bytes::copy_from_slice(&fixture.as_bytes()[..split]),
            Bytes::copy_from_slice(&fixture.as_bytes()[split..]),
        ];
        let server = SseServer::start(vec![ResponseSpec::chunks(chunks)]).await;
        let (_, rx) = channel(StreamCursor::default());
        let events =
            Client::tcp(server.address()).transcript_stream("ws", "sess", rx, exact_policy());
        pin_mut!(events);
        assert!(
            matches!(events.next().await, Some(TranscriptEvent::Snapshot(ref value)) if !value.reset)
        );
        assert!(
            matches!(events.next().await, Some(TranscriptEvent::Delta(ref value)) if value.cursor == 389)
        );
        assert!(
            matches!(events.next().await, Some(TranscriptEvent::Delta(ref value)) if value.cursor == 390)
        );
    }
}

#[tokio::test]
async fn ut_041_reset_fixture_maps_reason() {
    let server = SseServer::start(vec![ResponseSpec::sse(support::fixture(
        "stream_reset.sse",
    ))])
    .await;
    let (_, rx) = channel(StreamCursor::default());
    let events = Client::tcp(server.address()).transcript_stream("ws", "sess", rx, exact_policy());
    pin_mut!(events);
    assert!(
        matches!(events.next().await, Some(TranscriptEvent::Snapshot(ref value)) if value.reset && value.reason.as_deref() == Some("epoch_mismatch"))
    );
}

#[tokio::test]
async fn ut_042_first_connect_omits_zero_fences() {
    let server = SseServer::start(vec![ResponseSpec::Hang]).await;
    let (_, rx) = channel(StreamCursor::default());
    let events = Client::tcp(server.address()).transcript_stream("ws", "sess", rx, exact_policy());
    let task = tokio::spawn(async move {
        pin_mut!(events);
        events.next().await
    });
    wait_for_requests(&server, 1).await;
    let request = &server.requests()[0];
    assert_eq!(
        request.path_and_query,
        "/api/workspaces/ws/sessions/sess/stream?frames=transcript"
    );
    assert_eq!(request.last_event_id, None);
    task.abort();
}

#[tokio::test(start_paused = true)]
async fn ut_043_reconnect_sends_cursor_header_and_query() {
    let server = SseServer::start(vec![ResponseSpec::sse(delta(933)), ResponseSpec::Hang]).await;
    let (_, rx) = channel(StreamCursor::default());
    let events = Client::tcp(server.address()).transcript_stream("ws", "sess", rx, exact_policy());
    pin_mut!(events);
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Delta(_))
    ));
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Lost { .. })
    ));
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Reconnected)
    ));
    wait_for_requests(&server, 2).await;
    let request = &server.requests()[1];
    assert!(request.path_and_query.contains("after_sequence=933"));
    assert_eq!(request.last_event_id.as_deref(), Some("933"));
}

#[tokio::test(start_paused = true)]
async fn ut_044_stopped_ends_without_reconnect() {
    let server = SseServer::start(vec![ResponseSpec::sse(support::fixture(
        "session_stopped.sse",
    ))])
    .await;
    let (_, rx) = channel(StreamCursor::default());
    let events = Client::tcp(server.address()).transcript_stream("ws", "sess", rx, exact_policy());
    pin_mut!(events);
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Stopped(_))
    ));
    assert!(events.next().await.is_none());
    tokio::time::advance(Duration::from_secs(30)).await;
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn ut_045_unrelated_events_are_ignored() {
    let body = format!(
        "event: goal_snapshot_changed\ndata: {{}}\n\nevent: session_commands_changed\ndata: {{}}\n\n{}",
        snapshot(4, false, None)
    );
    let server = SseServer::start(vec![ResponseSpec::sse(body)]).await;
    let (_, rx) = channel(StreamCursor::default());
    let events = Client::tcp(server.address()).transcript_stream("ws", "sess", rx, exact_policy());
    pin_mut!(events);
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Snapshot(_))
    ));
}

#[tokio::test]
async fn ut_046_error_event_does_not_close_connection() {
    let body = format!(
        "event: error\ndata: {{\"error\":\"x\",\"code\":\"y\"}}\n\n{}",
        snapshot(5, false, None)
    );
    let server = SseServer::start(vec![ResponseSpec::sse(body)]).await;
    let (_, rx) = channel(StreamCursor::default());
    let events = Client::tcp(server.address()).transcript_stream("ws", "sess", rx, exact_policy());
    pin_mut!(events);
    assert!(
        matches!(events.next().await, Some(TranscriptEvent::ServerError(ref value)) if value.error == "x" && value.code.as_deref() == Some("y"))
    );
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Snapshot(_))
    ));
}

#[tokio::test(start_paused = true)]
async fn ut_047_keepalive_comments_reset_idle_timeout() {
    let server = SseServer::start(vec![ResponseSpec::timed(vec![
        (
            Duration::from_secs(30),
            Bytes::from_static(b": keepalive\n\n"),
        ),
        (
            Duration::from_secs(30),
            Bytes::from(snapshot(6, false, None)),
        ),
    ])])
    .await;
    let (_, rx) = channel(StreamCursor::default());
    let events = Client::tcp(server.address()).transcript_stream("ws", "sess", rx, exact_policy());
    pin_mut!(events);
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Snapshot(_))
    ));
}

#[tokio::test]
async fn ut_048_stopped_without_id_keeps_working() {
    let server = SseServer::start(vec![ResponseSpec::sse(support::fixture(
        "session_stopped.sse",
    ))])
    .await;
    let (_, rx) = channel(StreamCursor {
        after_sequence: 7,
        epoch: 1,
        generation: 2,
    });
    let events = Client::tcp(server.address()).transcript_stream("ws", "sess", rx, exact_policy());
    pin_mut!(events);
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Stopped(_))
    ));
    assert!(events.next().await.is_none());
}

#[tokio::test(start_paused = true)]
async fn ut_050_eof_emits_lost_with_initial_backoff() {
    let server = SseServer::start(vec![ResponseSpec::sse(snapshot(8, false, None))]).await;
    let (_, rx) = channel(StreamCursor::default());
    let events = Client::tcp(server.address()).transcript_stream("ws", "sess", rx, exact_policy());
    pin_mut!(events);
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Snapshot(_))
    ));
    assert!(
        matches!(events.next().await, Some(TranscriptEvent::Lost { attempt: 1, next_in, .. }) if next_in == Duration::from_millis(500))
    );
}

#[tokio::test(start_paused = true)]
async fn ut_051_consecutive_failures_exponentially_back_off_with_jitter() {
    let responses = (0..7)
        .map(|_| ResponseSpec::status(500, r#"{"error":"down"}"#))
        .collect();
    let server = SseServer::start(responses).await;
    let (_, rx) = channel(StreamCursor::default());
    let policy = ReconnectPolicy::default().with_seed(11);
    let events = Client::tcp(server.address()).transcript_stream("ws", "sess", rx, policy);
    pin_mut!(events);
    let bases = [0.5_f64, 1.0, 2.0, 4.0, 8.0, 10.0, 10.0];
    for (index, base) in bases.into_iter().enumerate() {
        let Some(TranscriptEvent::Lost {
            attempt, next_in, ..
        }) = events.next().await
        else {
            panic!("lost event")
        };
        assert_eq!(attempt, index as u32 + 1);
        assert!((base * 0.8..=base * 1.2).contains(&next_in.as_secs_f64()));
    }
}

#[tokio::test(start_paused = true)]
async fn ut_052_offline_threshold_does_not_stop_retries() {
    let responses = (0..6)
        .map(|_| ResponseSpec::status(503, r#"{"error":"down"}"#))
        .collect();
    let server = SseServer::start(responses).await;
    let (_, rx) = channel(StreamCursor::default());
    let events = Client::tcp(server.address()).transcript_stream("ws", "sess", rx, exact_policy());
    pin_mut!(events);
    for expected in 1..=6 {
        assert!(
            matches!(events.next().await, Some(TranscriptEvent::Lost { attempt, .. }) if attempt == expected)
        );
    }
}

#[tokio::test(start_paused = true)]
async fn ut_053_success_emits_reconnected_and_resets_attempt() {
    let server = SseServer::start(vec![
        ResponseSpec::status(500, r#"{"error":"down"}"#),
        ResponseSpec::sse(snapshot(9, false, None)),
    ])
    .await;
    let (_, rx) = channel(StreamCursor::default());
    let events = Client::tcp(server.address()).transcript_stream("ws", "sess", rx, exact_policy());
    pin_mut!(events);
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Lost { attempt: 1, .. })
    ));
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Reconnected)
    ));
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Snapshot(_))
    ));
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Lost { attempt: 1, .. })
    ));
}

#[tokio::test(start_paused = true)]
async fn ut_054_idle_connection_is_replaced() {
    let server = SseServer::start(vec![ResponseSpec::Hang]).await;
    let (_, rx) = channel(StreamCursor::default());
    let events = Client::tcp(server.address()).transcript_stream("ws", "sess", rx, exact_policy());
    pin_mut!(events);
    assert!(
        matches!(events.next().await, Some(TranscriptEvent::Lost { attempt: 1, ref error, .. }) if error.contains("idle timeout"))
    );
}

#[tokio::test(start_paused = true)]
async fn ut_055_reconnect_refreshes_fences_from_watch_source() {
    let server = SseServer::start(vec![
        ResponseSpec::sse(snapshot(10, true, Some("epoch_mismatch"))),
        ResponseSpec::Hang,
    ])
    .await;
    let (tx, rx) = channel(StreamCursor::default());
    let events = Client::tcp(server.address()).transcript_stream("ws", "sess", rx, exact_policy());
    pin_mut!(events);
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Snapshot(_))
    ));
    tx.send(StreamCursor {
        after_sequence: 10,
        epoch: 1,
        generation: 2,
    })
    .expect("refresh cursor");
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Lost { .. })
    ));
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Reconnected)
    ));
    wait_for_requests(&server, 2).await;
    let request = &server.requests()[1];
    assert!(request.path_and_query.contains("epoch=1"));
    assert!(request.path_and_query.contains("generation=2"));
    assert!(request.path_and_query.contains("after_sequence=10"));
}

#[tokio::test(start_paused = true)]
async fn ut_056_client_error_is_fatal() {
    let server = SseServer::start(vec![
        ResponseSpec::status(500, r#"{"error":"down"}"#),
        ResponseSpec::status(404, r#"{"error":"gone"}"#),
    ])
    .await;
    let (_, rx) = channel(StreamCursor::default());
    let events = Client::tcp(server.address()).transcript_stream("ws", "sess", rx, exact_policy());
    pin_mut!(events);
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Lost { .. })
    ));
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Fatal(Error::Daemon { status: 404, .. }))
    ));
    assert!(events.next().await.is_none());
}

#[tokio::test(start_paused = true)]
async fn ut_057_reconnect_while_draining_accepts_stream_success() {
    let server = SseServer::start(vec![
        ResponseSpec::status(503, r#"{"error":"daemon is draining"}"#),
        ResponseSpec::sse(snapshot(11, false, None)),
    ])
    .await;
    let (_, rx) = channel(StreamCursor::default());
    let events = Client::tcp(server.address()).transcript_stream("ws", "sess", rx, exact_policy());
    pin_mut!(events);
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Lost { .. })
    ));
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Reconnected)
    ));
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Snapshot(_))
    ));
}

#[tokio::test(start_paused = true)]
async fn ut_connect_response_headers_timeout_retries() {
    let server = SseServer::start(vec![ResponseSpec::HangBeforeHeaders]).await;
    let (_, rx) = channel(StreamCursor::default());
    let events = Client::tcp(server.address()).transcript_stream("ws", "sess", rx, exact_policy());
    pin_mut!(events);
    assert!(
        matches!(events.next().await, Some(TranscriptEvent::Lost { attempt: 1, ref error, .. }) if error.contains("idle timeout"))
    );
}

#[tokio::test(start_paused = true)]
async fn ut_058_reconnect_never_overlaps_an_open_connection() {
    let server = SseServer::start(vec![ResponseSpec::Hang, ResponseSpec::Hang]).await;
    let (_, rx) = channel(StreamCursor::default());
    let events = Client::tcp(server.address()).transcript_stream("ws", "sess", rx, exact_policy());
    pin_mut!(events);
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Lost { .. })
    ));
    assert!(server.active() <= 1);
    assert!(matches!(
        events.next().await,
        Some(TranscriptEvent::Reconnected)
    ));
    wait_for_requests(&server, 2).await;
    assert_eq!(server.max_active(), 1);
}
