mod support;

use compozy_client::{Client, Error, LogQuery};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

const WS: &str = "ws_e619d7250e618324";

fn client(server: &MockServer) -> Client {
    Client::tcp(server.address().to_string())
}

#[tokio::test]
async fn ut_315_overview_query_and_attention_shape() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/observe/overview"))
        .and(query_param("workspace", WS))
        .respond_with(ResponseTemplate::new(200).set_body_string(support::fixture("overview.json")))
        .mount(&server)
        .await;
    let overview = client(&server).overview(WS).await.unwrap();
    assert_eq!(
        overview.attention.total,
        overview.attention.items.len() as u64
    );
}

#[tokio::test]
async fn ut_316_overview_errors_preserve_503_and_404() {
    for status in [404, 503] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(status)
                    .set_body_json(serde_json::json!({"error":"unavailable"})),
            )
            .mount(&server)
            .await;
        assert!(
            matches!(client(&server).overview(WS).await, Err(Error::Daemon { status: actual, .. }) if actual == status)
        );
    }
}

#[tokio::test]
async fn ut_317_logs_use_workspace_id_and_decode_events() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/logs"))
        .and(query_param("workspace_id", WS))
        .and(query_param("session_id", "sess-1"))
        .and(query_param("error_only", "true"))
        .and(query_param("limit", "200"))
        .respond_with(ResponseTemplate::new(200).set_body_string(support::fixture("logs.json")))
        .mount(&server)
        .await;
    let events = client(&server)
        .logs(&LogQuery {
            workspace_id: WS,
            session_id: Some("sess-1"),
            run: None,
            error_only: true,
            after_seq: None,
            limit: 200,
        })
        .await
        .unwrap();
    assert!(!events.is_empty());
    let request = server.received_requests().await.unwrap().remove(0);
    assert!(!request.url.query().unwrap().contains("workspace="));
}

#[tokio::test]
async fn ut_318_logs_support_run_scope() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/logs"))
        .and(query_param("run", "looprun-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"events":[]})))
        .mount(&server)
        .await;
    client(&server)
        .logs(&LogQuery {
            workspace_id: WS,
            session_id: None,
            run: Some("looprun-1"),
            error_only: false,
            after_seq: None,
            limit: 0,
        })
        .await
        .unwrap();
}
