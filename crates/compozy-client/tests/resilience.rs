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
    let mut reconnect_policy = ReconnectPolicy::default();
    reconnect_policy.jitter = 0.0;
    reconnect_policy.idle_timeout = Duration::from_millis(100);
    let min_backoff = reconnect_policy.min;
    let max_backoff = reconnect_policy.max;

    let client = Client::tcp(daemon.tcp_addr().to_string());
    let (_, receiver) = watch::channel(NoCursor);
    let requests = daemon.request_log();
    requests.clear();
    let stream = client.catalog_stream(receiver, reconnect_policy);
    futures_util::pin_mut!(stream);

    let started = Instant::now();
    for flap in 0..5 {
        daemon.set_catalog_draining(true);

        let first_lost = tokio::time::timeout(Duration::from_secs(2), stream.next())
            .await
            .expect("first retry event timeout")
            .expect("catalog stream ended");
        let StreamEvent::Lost {
            attempt: 1,
            next_in: first_delay,
            ..
        } = first_lost
        else {
            panic!("flap {flap}: expected first lost event, got {first_lost:?}");
        };
        assert!((min_backoff..=max_backoff).contains(&first_delay));

        let retry_started = Instant::now();
        let second_lost = tokio::time::timeout(first_delay + Duration::from_secs(1), stream.next())
            .await
            .expect("second retry event timeout")
            .expect("catalog stream ended");
        let StreamEvent::Lost {
            attempt: 2,
            next_in: second_delay,
            ..
        } = second_lost
        else {
            panic!("flap {flap}: expected second lost event, got {second_lost:?}");
        };
        assert!(retry_started.elapsed() >= first_delay);
        assert!((min_backoff..=max_backoff).contains(&second_delay));

        daemon.set_catalog_draining(false);
        let recovery_started = Instant::now();
        let recovered = tokio::time::timeout(second_delay + Duration::from_secs(1), stream.next())
            .await
            .expect("catalog recovery timeout")
            .expect("catalog stream ended");
        assert!(
            matches!(recovered, StreamEvent::Reconnected),
            "flap {flap}: expected reconnect, got {recovered:?}"
        );
        assert!(recovery_started.elapsed() >= second_delay);
    }

    assert!(started.elapsed() < Duration::from_secs(10));
    let catalog_requests = requests
        .entries()
        .into_iter()
        .filter(|(method, path)| method == "GET" && path == "/api/sessions/catalog-stream")
        .count();
    assert_eq!(catalog_requests, 11, "unexpected reconnect attempts");
}
