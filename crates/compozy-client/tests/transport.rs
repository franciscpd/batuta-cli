mod support;
#[path = "support/uds.rs"]
mod uds;

use compozy_client::{Client, Error, Outcome, Transport, TransportOrder};
use std::{path::PathBuf, time::Duration};
use uds::{ResponseSpec, UdsServer, status_response};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

async fn status_mock() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/status"))
        .respond_with(ResponseTemplate::new(200).set_body_string(support::fixture("status.json")))
        .mount(&server)
        .await;
    server
}

fn missing_socket(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("compozy-client-{name}-{}.sock", std::process::id()))
}

#[tokio::test]
async fn ut_001_probe_auto_chooses_uds_and_skips_tcp() {
    let uds = UdsServer::start(vec![status_response()]).await;
    let (report, client) = Client::probe(
        TransportOrder::Auto,
        uds.path.clone(),
        "127.0.0.1:1",
        Duration::from_secs(3),
    )
    .await;
    assert_eq!(report.chosen, Some(Transport::Uds(uds.path.clone())));
    assert_eq!(report.targets[0].outcome, Outcome::Ok);
    assert_eq!(report.targets[1].outcome, Outcome::Skipped);
    assert!(client.is_some());
}

#[tokio::test]
async fn ut_002_probe_falls_back_from_missing_uds_to_tcp() {
    let tcp = status_mock().await;
    let missing = missing_socket("missing-fallback");
    let (report, _) = Client::probe(
        TransportOrder::Auto,
        missing.clone(),
        &tcp.address().to_string(),
        Duration::from_secs(3),
    )
    .await;
    assert_eq!(
        report.chosen,
        Some(Transport::Tcp(tcp.address().to_string()))
    );
    assert_eq!(
        report.targets[0].outcome,
        Outcome::Error(format!("not found: {}", missing.display()))
    );
    assert_eq!(report.targets[1].outcome, Outcome::Ok);
}

#[tokio::test]
async fn ut_003_stale_socket_reports_connection_refused_then_tries_tcp() {
    let tcp = status_mock().await;
    let path = missing_socket("stale");
    let listener = tokio::net::UnixListener::bind(&path).expect("bind stale socket");
    drop(listener);
    let (report, _) = Client::probe(
        TransportOrder::Auto,
        path.clone(),
        &tcp.address().to_string(),
        Duration::from_secs(3),
    )
    .await;
    assert_eq!(
        report.targets[0].outcome,
        Outcome::Error("connection refused".into())
    );
    assert_eq!(report.targets[1].outcome, Outcome::Ok);
    std::fs::remove_file(path).expect("remove stale socket");
}

#[tokio::test]
async fn ut_004_missing_socket_reports_path() {
    let missing = missing_socket("missing-only");
    let (report, _) = Client::probe(
        TransportOrder::UdsOnly,
        missing.clone(),
        "127.0.0.1:1",
        Duration::from_secs(3),
    )
    .await;
    assert_eq!(
        report.targets[0].outcome,
        Outcome::Error(format!("not found: {}", missing.display()))
    );
}

#[tokio::test(start_paused = true)]
async fn ut_005_each_probe_target_has_its_own_timeout() {
    let uds = UdsServer::start(vec![status_response().delayed(Duration::from_secs(4))]).await;
    let tcp = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/status"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(4))
                .set_body_string(support::fixture("status.json")),
        )
        .mount(&tcp)
        .await;
    let (report, client) = Client::probe(
        TransportOrder::Auto,
        uds.path.clone(),
        &tcp.address().to_string(),
        Duration::from_secs(3),
    )
    .await;
    assert_eq!(report.targets[0].outcome, Outcome::Error("timeout".into()));
    assert_eq!(report.targets[1].outcome, Outcome::Error("timeout".into()));
    assert!(report.chosen.is_none());
    assert!(client.is_none());
}

#[tokio::test]
async fn ut_006_uds_only_never_touches_tcp() {
    let missing = missing_socket("uds-only");
    let (report, _) = Client::probe(
        TransportOrder::UdsOnly,
        missing,
        "127.0.0.1:1",
        Duration::from_secs(3),
    )
    .await;
    assert_eq!(report.targets[1].outcome, Outcome::Skipped);
}

#[tokio::test]
async fn ut_007_tcp_only_never_touches_uds() {
    let tcp = status_mock().await;
    let (report, _) = Client::probe(
        TransportOrder::TcpOnly,
        missing_socket("tcp-only"),
        &tcp.address().to_string(),
        Duration::from_secs(3),
    )
    .await;
    assert_eq!(report.targets[0].outcome, Outcome::Skipped);
    assert_eq!(report.targets[1].outcome, Outcome::Ok);
}

#[tokio::test]
async fn ut_008_uds_request_uses_localhost_host_and_status_path() {
    let uds = UdsServer::start(vec![status_response()]).await;
    Client::uds(uds.path.clone())
        .status()
        .await
        .expect("status");
    let requests = uds.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path_and_query, "/api/status");
    assert_eq!(requests[0].host.as_deref(), Some("localhost"));
    assert!(requests[0].body.is_empty());
}

#[tokio::test(start_paused = true)]
async fn ut_009_single_shot_request_times_out_after_ten_seconds() {
    let uds = UdsServer::start(vec![ResponseSpec::hanging()]).await;
    let error = Client::uds(uds.path.clone())
        .status()
        .await
        .expect_err("timeout");
    assert!(matches!(error, Error::Timeout(duration) if duration == Duration::from_secs(10)));
}

#[tokio::test]
async fn ut_010_malformed_status_is_unexpected_payload() {
    let uds = UdsServer::start(vec![ResponseSpec::json(200, "not json")]).await;
    assert!(matches!(
        Client::uds(uds.path.clone()).status().await,
        Err(Error::UnexpectedPayload(200))
    ));
}

#[tokio::test]
async fn ut_011_draining_status_is_synthesized_as_success() {
    let uds = UdsServer::start(vec![ResponseSpec::json(
        503,
        r#"{"error":"daemon is draining; new work admission is closed"}"#,
    )])
    .await;
    let status = Client::uds(uds.path.clone())
        .status()
        .await
        .expect("draining status");
    assert_eq!(status.daemon.status, "draining");
}

#[tokio::test]
async fn ut_012_fixed_route_404_is_route_missing() {
    let uds = UdsServer::start(vec![ResponseSpec::json(404, r#"{"error":"missing"}"#)]).await;
    assert!(matches!(
        Client::uds(uds.path.clone()).workspaces().await,
        Err(Error::RouteMissing { method: "GET", path }) if path == "/api/workspaces"
    ));
}

#[tokio::test]
async fn ut_013_daemon_error_preserves_envelope_and_display() {
    let uds = UdsServer::start(vec![ResponseSpec::json(
        500,
        r#"{"error":"boom","code":"x_failed"}"#,
    )])
    .await;
    let error = Client::uds(uds.path.clone())
        .workspaces()
        .await
        .expect_err("daemon error");
    assert_eq!(error.to_string(), "boom (x_failed)");
    assert!(matches!(
        error,
        Error::Daemon { status: 500, message, code: Some(code), .. }
            if message == "boom" && code == "x_failed"
    ));
}
