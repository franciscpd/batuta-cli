use compozy_client::{Client, Error, types::ClarifyAnswer};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

const WORKSPACE: &str = "ws_e619d7250e618324";
const SESSION: &str = "sess_1";

fn client(server: &MockServer) -> Client {
    Client::tcp(server.address().to_string())
}

#[tokio::test]
async fn ut_319_clarifications_decodes_pending_items_and_empty_list() {
    let route = format!("/api/workspaces/{WORKSPACE}/sessions/{SESSION}/clarifications");
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(route.as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"clarifications":[{"request_id":"req_1","session_id":"sess_1","agent_name":"code_implementer","question":"Which environment?","choices":["staging","production"],"asked_at":"2026-08-18T10:00:00Z","deadline":"2026-08-18T10:05:00Z"}]}"#,
            "application/json",
        ))
        .mount(&server)
        .await;
    let pending = client(&server)
        .clarifications(WORKSPACE, SESSION)
        .await
        .expect("clarifications");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].request_id, "req_1");
    assert_eq!(pending[0].choices, ["staging", "production"]);

    let empty = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(route.as_str()))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(r#"{"clarifications":[]}"#, "application/json"),
        )
        .mount(&empty)
        .await;
    assert!(
        client(&empty)
            .clarifications(WORKSPACE, SESSION)
            .await
            .expect("empty clarifications")
            .is_empty()
    );
}

#[tokio::test]
async fn ut_321_clarification_answer_maps_404_409_and_503() {
    let route =
        format!("/api/workspaces/{WORKSPACE}/sessions/{SESSION}/clarifications/req_1/answer");
    for status in [404, 409, 503] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(route.as_str()))
            .respond_with(ResponseTemplate::new(status).set_body_raw(
                format!(r#"{{"error":"clarify {status}"}}"#),
                "application/json",
            ))
            .mount(&server)
            .await;
        assert!(matches!(
            client(&server)
                .answer_clarification(
                    WORKSPACE,
                    SESSION,
                    "req_1",
                    &ClarifyAnswer::Text("x".to_owned()),
                )
                .await,
            Err(Error::Daemon { status: actual, .. }) if actual == status
        ));
    }
}
