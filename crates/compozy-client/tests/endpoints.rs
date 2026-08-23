mod support;

use compozy_client::{Client, Error, SessionQuery, TranscriptQuery};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

async fn server_with(path_value: &str, status: u16, body: impl Into<String>) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(path_value))
        .respond_with(ResponseTemplate::new(status).set_body_string(body))
        .mount(&server)
        .await;
    server
}

fn client(server: &MockServer) -> Client {
    Client::tcp(server.address().to_string())
}

fn session_query<'a>() -> SessionQuery<'a> {
    SessionQuery {
        workspace: "ws_e619d7250e618324",
        type_: Some("user"),
        sort: "recent",
        limit: 20,
        agent: Some("batuta"),
    }
}

#[tokio::test]
async fn ut_020_workspaces_decodes_once_without_pagination() {
    let server = server_with("/api/workspaces", 200, support::fixture("workspaces.json")).await;
    let workspaces = client(&server).workspaces().await.expect("workspaces");
    assert_eq!(workspaces.len(), 3);
    assert!(workspaces.iter().all(|workspace| {
        !workspace.id.is_empty() && !workspace.name.is_empty() && !workspace.root_dir.is_empty()
    }));
    assert_eq!(server.received_requests().await.expect("requests").len(), 1);
}

#[tokio::test]
async fn ut_021_sessions_sends_exact_query_and_decodes_page() {
    let server = server_with("/api/sessions", 200, support::fixture("sessions.json")).await;
    let page = client(&server)
        .sessions(&session_query())
        .await
        .expect("sessions");
    assert_eq!(page.sessions.len(), 1);
    assert!(!page.page.has_more);
    assert_eq!(page.page.total, Some(1));
    assert_eq!(page.page.limit, 5);
    let requests = server.received_requests().await.expect("requests");
    assert_eq!(
        requests[0].url.query(),
        Some("workspace=ws_e619d7250e618324&sort=recent&limit=20&type=user&agent=batuta")
    );
}

#[tokio::test]
async fn ut_022_session_decodes_wrapped_payload() {
    let server = server_with(
        "/api/workspaces/ws_e619d7250e618324/sessions/sess_1",
        200,
        support::fixture("session.json"),
    )
    .await;
    let session = client(&server)
        .session("ws_e619d7250e618324", "sess_1")
        .await
        .expect("session");
    assert_eq!(session.state, "active");
    assert_eq!(session.agent_name, "code_implementer");
}

#[tokio::test]
async fn ut_023_transcript_without_cursor_sends_limit_only_and_decodes() {
    let route = "/api/workspaces/ws_e619d7250e618324/sessions/sess_1/transcript";
    let server = server_with(route, 200, support::fixture("transcript_page.json")).await;
    let page = client(&server)
        .transcript(
            "ws_e619d7250e618324",
            "sess_1",
            &TranscriptQuery {
                limit: 200,
                before_sequence: None,
            },
        )
        .await
        .expect("transcript");
    assert_eq!(page.epoch, 0);
    assert_eq!(page.generation, 0);
    assert_eq!(page.max_sequence, 388);
    assert!(!page.has_older);
    assert_eq!(page.next_before_sequence, None);
    let requests = server.received_requests().await.expect("requests");
    assert_eq!(requests[0].url.query(), Some("limit=200"));
}

#[tokio::test]
async fn ut_024_transcript_cursor_is_appended_after_limit() {
    let route = "/api/workspaces/ws_e619d7250e618324/sessions/sess_1/transcript";
    let server = server_with(route, 200, support::fixture("transcript_page.json")).await;
    client(&server)
        .transcript(
            "ws_e619d7250e618324",
            "sess_1",
            &TranscriptQuery {
                limit: 200,
                before_sequence: Some(778),
            },
        )
        .await
        .expect("transcript");
    let requests = server.received_requests().await.expect("requests");
    assert_eq!(
        requests[0].url.query(),
        Some("limit=200&before_sequence=778")
    );
}

#[tokio::test]
async fn ut_025_sessions_workspace_404_remains_daemon_error() {
    let server = server_with("/api/sessions", 404, r#"{"error":"workspace not found"}"#).await;
    assert!(matches!(
        client(&server).sessions(&session_query()).await,
        Err(Error::Daemon { status: 404, message, .. }) if message == "workspace not found"
    ));
}
