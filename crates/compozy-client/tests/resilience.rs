use compozy_client::{Client, NoCursor, ReconnectPolicy, StreamEvent};
use compozy_testkit::{Daemon, StartOutcome};
use futures_util::StreamExt;
use std::time::{Duration, Instant};
use tokio::sync::watch;

fn policy() -> ReconnectPolicy {
    let mut policy = ReconnectPolicy::default();
    policy.min = Duration::from_millis(100);
    policy.max = Duration::from_millis(400);
    policy.jitter = 0.0;
    policy.idle_timeout = Duration::from_secs(5);
    policy.offline_after = 5;
    policy
}

async fn daemon() -> Option<Box<Daemon>> {
    match Daemon::start().await.expect("start disposable daemon") {
        StartOutcome::Started(daemon) => Some(daemon),
        StartOutcome::Skip(reason) => {
            eprintln!("skipped: {reason}");
            None
        }
    }
}

#[tokio::test]
async fn it_009_catalog_reconnects_via_sse_after_a_503() {
    let Some(daemon) = daemon().await else { return };
    daemon.set_catalog_draining(true);
    let client = Client::tcp(daemon.tcp_addr().to_string());
    let (_, receiver) = watch::channel(NoCursor);
    let stream = client.catalog_stream(receiver, policy());
    futures_util::pin_mut!(stream);

    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), stream.next()).await,
        Ok(Some(StreamEvent::Lost { attempt: 1, .. }))
    ));
    tokio::time::sleep(Duration::from_secs(3)).await;
    daemon.set_catalog_draining(false);
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match stream.next().await.expect("catalog stream ended") {
                StreamEvent::Reconnected => break,
                StreamEvent::Lost { .. } => {}
                StreamEvent::Fatal(error) => panic!("catalog became fatal: {error}"),
                _ => {}
            }
        }
    })
    .await
    .expect("catalog recovery timeout");
}

#[tokio::test]
async fn it_010_recovered_catalog_delivers_at_sse_latency() {
    let Some(daemon) = daemon().await else { return };
    daemon.set_catalog_draining(true);
    let client = Client::tcp(daemon.tcp_addr().to_string());
    let (_, receiver) = watch::channel(NoCursor);
    let stream = client.catalog_stream(receiver, policy());
    futures_util::pin_mut!(stream);

    assert!(matches!(
        stream.next().await,
        Some(StreamEvent::Lost { .. })
    ));
    daemon.set_catalog_draining(false);
    tokio::time::timeout(Duration::from_secs(2), async {
        while !matches!(stream.next().await, Some(StreamEvent::Reconnected)) {}
    })
    .await
    .expect("catalog recovery timeout");

    let started = Instant::now();
    let session_id = daemon
        .create_session("batuta")
        .await
        .expect("create session");
    let event = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(StreamEvent::Event(event)) = stream.next().await {
                break event;
            }
        }
    })
    .await
    .expect("catalog event exceeded SSE latency bound");
    assert_eq!(event.session_id, session_id);
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[tokio::test]
async fn it_011_catalog_flapping_obeys_reconnect_backoff() {
    let Some(daemon) = daemon().await else { return };
    daemon.set_catalog_draining(true);
    let client = Client::tcp(daemon.tcp_addr().to_string());
    let (_, receiver) = watch::channel(NoCursor);
    let stream = client.catalog_stream(receiver, policy());
    futures_util::pin_mut!(stream);

    let mut prior = Instant::now();
    for expected in [
        Duration::ZERO,
        Duration::from_millis(100),
        Duration::from_millis(200),
        Duration::from_millis(400),
        Duration::from_millis(400),
    ] {
        let event = tokio::time::timeout(Duration::from_secs(2), stream.next())
            .await
            .expect("retry event timeout")
            .expect("catalog stream ended");
        let elapsed = prior.elapsed();
        prior = Instant::now();
        assert!(matches!(event, StreamEvent::Lost { .. }));
        if !expected.is_zero() {
            assert!(elapsed >= expected.saturating_sub(Duration::from_millis(25)));
            assert!(elapsed <= expected + Duration::from_millis(250));
        }
    }
}
