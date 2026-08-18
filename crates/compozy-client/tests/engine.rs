#[path = "support/sse_server.rs"]
mod sse_server;

use compozy_client::{
    Client, Cursor, Error, LogCursor, NoCursor, ReconnectPolicy, SeqCursor, StreamEvent,
    sse::{EventStream, Frame, StreamRequest},
};
use futures_util::{StreamExt, pin_mut};
use sse_server::{ResponseSpec, SseServer};
use std::time::Duration;
use tokio::sync::watch;

fn policy() -> ReconnectPolicy {
    let mut value = ReconnectPolicy::default().with_seed(7);
    value.jitter = 0.0;
    value
}

fn json(frame: &Frame) -> Option<serde_json::Value> {
    serde_json::from_str(&frame.data).ok()
}

async fn wait_for_requests(server: &SseServer, count: usize) {
    for _ in 0..200 {
        if server.requests().len() >= count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("expected {count} requests, got {}", server.requests().len());
}

#[tokio::test(start_paused = true)]
async fn ut_312_loop_events_reconnect_with_seq_header_query_and_kind() {
    let event = "id: 6\nevent: node_running\ndata: {\"id\":\"e6\",\"seq\":6,\"kind\":\"node_running\",\"payload\":{},\"at\":\"2026-08-18T00:00:00Z\"}\n\n";
    let detail = r#"{"run":{"id":"run","loop_name":"x","status":"running","generation":1,"created_at":"2026-08-18T00:00:00Z","started_at":"2026-08-18T00:00:00Z","last_progress_at":"2026-08-18T00:00:00Z"},"materialized_contract":{},"generations":[],"node_controls":[],"waits":[]}"#;
    let server = SseServer::start(vec![
        ResponseSpec::status(200, detail),
        ResponseSpec::sse(event),
        ResponseSpec::Hang,
    ])
    .await;
    let (_, rx) = watch::channel(SeqCursor::default());
    let stream = Client::tcp(server.address()).loop_run_events("ws", "run", rx, policy());
    pin_mut!(stream);
    assert!(
        matches!(stream.next().await, Some(StreamEvent::Event(ref event)) if event.seq == 6 && event.kind == "node_running")
    );
    assert!(matches!(
        stream.next().await,
        Some(StreamEvent::Lost { .. })
    ));
    assert!(matches!(
        stream.next().await,
        Some(StreamEvent::Reconnected)
    ));
    wait_for_requests(&server, 3).await;
    assert_eq!(server.requests()[0].last_event_id, None);
    assert_eq!(server.requests()[1].last_event_id, None);
    assert_eq!(server.requests()[2].last_event_id.as_deref(), Some("6"));
    assert!(
        server.requests()[2]
            .path_and_query
            .contains("after_sequence=6")
    );
}

#[tokio::test(start_paused = true)]
async fn ut_320_logs_stream_composite_cursor_and_unnamed_event() {
    let id = "2026-08-18T00:34:02.360Z|00000000000000000042";
    let body = format!(
        "id: {id}\ndata: {{\"id\":\"log-1\",\"session_id\":\"s\",\"type\":\"error\",\"agent_name\":\"a\",\"timestamp\":\"2026-08-18T00:34:02.360Z\"}}\n\n"
    );
    let server = SseServer::start(vec![ResponseSpec::sse(body), ResponseSpec::Hang]).await;
    let (_, rx) = watch::channel(LogCursor::default());
    let query = compozy_client::LogQuery {
        workspace_id: "ws",
        session_id: None,
        run: None,
        error_only: false,
        after_seq: Some(99),
        limit: 200,
    };
    let stream = Client::tcp(server.address()).logs_stream(&query, rx, policy());
    pin_mut!(stream);
    assert!(
        matches!(stream.next().await, Some(StreamEvent::Event(ref event)) if event.id == "log-1")
    );
    assert!(matches!(
        stream.next().await,
        Some(StreamEvent::Lost { .. })
    ));
    assert!(matches!(
        stream.next().await,
        Some(StreamEvent::Reconnected)
    ));
    wait_for_requests(&server, 2).await;
    assert_eq!(server.requests()[1].last_event_id.as_deref(), Some(id));
    assert!(
        server
            .requests()
            .iter()
            .all(|request| !request.path_and_query.contains("after_seq"))
    );
}

#[tokio::test]
async fn ut_322_catalog_event_ignores_ready_comment() {
    let server = SseServer::start(vec![ResponseSpec::sse(": session catalog stream ready\n\nevent: session_catalog_changed\ndata: {\"kind\":\"upserted\",\"workspace_id\":\"ws\",\"session_id\":\"s\"}\n\n")]).await;
    let (_, rx) = watch::channel(NoCursor);
    let stream = Client::tcp(server.address()).catalog_stream(rx, policy());
    pin_mut!(stream);
    assert!(
        matches!(stream.next().await, Some(StreamEvent::Event(ref event)) if event.kind == "upserted" && event.workspace_id == "ws")
    );
}

#[tokio::test]
async fn ut_323_catalog_503_is_fatal() {
    let server = SseServer::start(vec![ResponseSpec::status(
        503,
        r#"{"error":"unavailable"}"#,
    )])
    .await;
    let (_, rx) = watch::channel(NoCursor);
    let stream = Client::tcp(server.address()).catalog_stream(rx, policy());
    pin_mut!(stream);
    assert!(matches!(
        stream.next().await,
        Some(StreamEvent::Fatal(Error::Daemon { status: 503, .. }))
    ));
}

#[tokio::test(start_paused = true)]
async fn ut_330_no_cursor_never_sends_header_or_query() {
    let server =
        SseServer::start(vec![ResponseSpec::sse("data: {}\n\n"), ResponseSpec::Hang]).await;
    let (_, rx) = watch::channel(NoCursor);
    let stream = EventStream::open(
        Client::tcp(server.address()),
        StreamRequest::new("/events", "test"),
        rx,
        policy(),
        json,
    );
    pin_mut!(stream);
    assert!(matches!(stream.next().await, Some(StreamEvent::Event(_))));
    assert!(matches!(
        stream.next().await,
        Some(StreamEvent::Lost { .. })
    ));
    assert!(matches!(
        stream.next().await,
        Some(StreamEvent::Reconnected)
    ));
    wait_for_requests(&server, 2).await;
    assert!(
        server
            .requests()
            .iter()
            .all(|request| request.last_event_id.is_none() && request.path_and_query == "/events")
    );
}

#[tokio::test(start_paused = true)]
async fn ut_331_seq_cursor_applies_ids_and_rereads_watch() {
    let server = SseServer::start(vec![
        ResponseSpec::sse("id: 6\ndata: {}\n\n"),
        ResponseSpec::Hang,
    ])
    .await;
    let (tx, rx) = watch::channel(SeqCursor::default());
    let stream = EventStream::open(
        Client::tcp(server.address()),
        StreamRequest::new("/events", "test"),
        rx,
        policy(),
        json,
    );
    pin_mut!(stream);
    assert!(matches!(stream.next().await, Some(StreamEvent::Event(_))));
    tx.send(SeqCursor(99)).unwrap();
    assert!(matches!(
        stream.next().await,
        Some(StreamEvent::Lost { .. })
    ));
    assert!(matches!(
        stream.next().await,
        Some(StreamEvent::Reconnected)
    ));
    wait_for_requests(&server, 2).await;
    assert_eq!(server.requests()[1].last_event_id.as_deref(), Some("99"));
}

#[test]
fn ut_332_log_cursor_round_trips_composite_id() {
    let id = "2026-08-18T00:34:02.360Z|00000000000000000042";
    let mut cursor = LogCursor::default();
    cursor.apply_id(id);
    assert_eq!(cursor.ts, "2026-08-18T00:34:02.360Z");
    assert_eq!(cursor.seq, 42);
    assert_eq!(cursor.last_event_id().as_deref(), Some(id));
}

#[tokio::test]
async fn ut_333_transcript_wrapper_preserves_typed_snapshot_events() {
    let body = "event: transcript_snapshot\ndata: {\"session_id\":\"s\",\"epoch\":1,\"generation\":1,\"entries\":[],\"max_sequence\":0,\"reset\":false}\n\n";
    let server = SseServer::start(vec![ResponseSpec::sse(body)]).await;
    let (_, rx) = watch::channel(compozy_client::StreamCursor::default());
    let stream = Client::tcp(server.address()).transcript_stream("ws", "s", rx, policy());
    pin_mut!(stream);
    assert!(
        matches!(stream.next().await, Some(compozy_client::TranscriptEvent::Snapshot(snapshot)) if !snapshot.reset)
    );
}

#[test]
fn ut_334_all_cursor_types_share_the_same_policy_defaults() {
    let policy = ReconnectPolicy::default();
    assert_eq!(policy.min, Duration::from_millis(500));
    assert_eq!(policy.max, Duration::from_secs(10));
    assert_eq!(policy.idle_timeout, Duration::from_secs(60));
    assert_eq!(NoCursor.query(), Vec::<(&'static str, String)>::new());
    assert_eq!(SeqCursor(1).query(), vec![("after_sequence", "1".into())]);
    assert!(LogCursor::default().query().is_empty());
    let _ = ResponseSpec::chunks(Vec::new());
    let _ = ResponseSpec::timed(Vec::new());
    let _ = ResponseSpec::HangBeforeHeaders;
}

#[tokio::test]
async fn ut_335_dropping_stream_aborts_open_connection() {
    let server = SseServer::start(vec![ResponseSpec::Hang]).await;
    let (_, rx) = watch::channel(NoCursor);
    let stream = EventStream::open(
        Client::tcp(server.address()),
        StreamRequest::new("/events", "test"),
        rx,
        policy(),
        json,
    );
    let task = tokio::spawn(async move {
        pin_mut!(stream);
        stream.next().await
    });
    wait_for_requests(&server, 1).await;
    assert_eq!(server.max_active(), 1);
    task.abort();
    for _ in 0..10 {
        if server.active() == 0 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("connection remained open after stream drop");
}

#[tokio::test(start_paused = true)]
async fn ut_336_client_errors_are_fatal_server_errors_retry() {
    let server = SseServer::start(vec![
        ResponseSpec::status(500, r#"{"error":"down"}"#),
        ResponseSpec::status(404, r#"{"error":"gone"}"#),
    ])
    .await;
    let (_, rx) = watch::channel(NoCursor);
    let stream = EventStream::open(
        Client::tcp(server.address()),
        StreamRequest::new("/events", "test"),
        rx,
        policy(),
        json,
    );
    pin_mut!(stream);
    assert!(matches!(
        stream.next().await,
        Some(StreamEvent::Lost { .. })
    ));
    assert!(matches!(
        stream.next().await,
        Some(StreamEvent::Fatal(Error::Daemon { status: 404, .. }))
    ));
}
