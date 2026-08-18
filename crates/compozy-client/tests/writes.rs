#[path = "support/sse_server.rs"]
pub mod sse_server;
mod support;
#[path = "support/uds.rs"]
pub mod uds;

use compozy_client::{
    Client, Error, TaskVerb,
    types::{
        ApproveRequest, ClarifyAnswer, Decision, PromptMode, PromptOutcome, PromptRequest,
        PromptRuntime,
    },
};
use serde_json::{Value, json};
use std::{fs, path::PathBuf, time::Duration};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

const WORKSPACE: &str = "ws_e619d7250e618324";
const SESSION: &str = "sess_1";

fn client(server: &MockServer) -> Client {
    Client::tcp(server.address().to_string())
}

async fn mock_json(path_value: &str, status: u16, body: impl Into<String>) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(path_value))
        .respond_with(
            ResponseTemplate::new(status)
                .insert_header("content-type", "application/json")
                .set_body_string(body),
        )
        .mount(&server)
        .await;
    server
}

fn prompt_request() -> PromptRequest {
    PromptRequest {
        message: "ship it".to_owned(),
        message_id: "msg_0123456789abcdef".to_owned(),
        idempotency_key: "idem_0123456789abcdef".to_owned(),
        mode: PromptMode::Queue,
        expected_turn_id: None,
        runtime: None,
    }
}

#[tokio::test]
async fn ut_300_create_session_sends_exact_body_and_decodes_201() {
    let server = mock_json(
        "/api/sessions",
        201,
        support::fixture("session_created.json"),
    )
    .await;
    let session = client(&server)
        .create_session(WORKSPACE, "batuta")
        .await
        .expect("create session");
    assert_eq!(session.id, "sess_created_1");

    let requests = server.received_requests().await.expect("requests");
    assert_eq!(
        requests[0]
            .headers
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("application/json")
    );
    let body: Value = serde_json::from_slice(&requests[0].body).expect("JSON body");
    assert_eq!(
        body,
        json!({"workspace": WORKSPACE, "agent_name": "batuta"})
    );
}

#[tokio::test]
async fn ut_301_approve_permission_uses_all_four_decision_wire_strings() {
    let route = format!("/api/workspaces/{WORKSPACE}/sessions/{SESSION}/approve");
    let server = mock_json(&route, 200, support::fixture("approve_200.json")).await;
    let cases = [
        (Decision::AllowOnce, "allow-once"),
        (Decision::AllowAlways, "allow-always"),
        (Decision::RejectOnce, "reject-once"),
        (Decision::RejectAlways, "reject-always"),
    ];
    for (decision, _) in cases {
        client(&server)
            .approve_permission(
                WORKSPACE,
                SESSION,
                &ApproveRequest {
                    request_id: "req_1".to_owned(),
                    turn_id: "turn-1".to_owned(),
                    decision,
                },
            )
            .await
            .expect("approve permission");
    }

    let requests = server.received_requests().await.expect("requests");
    assert_eq!(requests.len(), cases.len());
    for (request, (_, expected)) in requests.iter().zip(cases) {
        let body: Value = serde_json::from_slice(&request.body).expect("JSON body");
        assert_eq!(body["decision"], expected);
    }
}

#[tokio::test]
async fn ut_302_answer_clarification_sends_exactly_one_key() {
    let route =
        format!("/api/workspaces/{WORKSPACE}/sessions/{SESSION}/clarifications/req_1/answer");
    let server = mock_json(&route, 200, support::fixture("clarify_200.json")).await;
    let answers = [
        ClarifyAnswer::Choice(2),
        ClarifyAnswer::Text("x".to_owned()),
    ];
    for answer in &answers {
        let result = client(&server)
            .answer_clarification(WORKSPACE, SESSION, "req_1", answer)
            .await
            .expect("answer clarification");
        assert_eq!(result.choice, Some(2));
    }

    let requests = server.received_requests().await.expect("requests");
    assert_eq!(requests.len(), 2);
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[0].body).unwrap(),
        json!({"choice_index": 2})
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[1].body).unwrap(),
        json!({"text": "x"})
    );
}

#[tokio::test]
async fn ut_303_task_verbs_use_scoped_routes_and_pinned_enqueue_retry() {
    let server = MockServer::start().await;
    let approve = format!("/api/workspaces/{WORKSPACE}/tasks/task_1/approve");
    let reject = format!("/api/workspaces/{WORKSPACE}/tasks/task_1/reject");
    let routes = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contract/routes.txt"),
    )
    .expect("routes file");
    let retry_template = routes
        .lines()
        .find_map(|line| {
            line.strip_prefix("POST /api/tasks/")
                .map(|path| format!("/api/tasks/{path}"))
        })
        .expect("pinned retry route");
    assert_eq!(retry_template, "/api/tasks/{id}/runs");
    let retry = retry_template.replace("{id}", "task_1");

    for route in [&approve, &reject, &retry] {
        Mock::given(method("POST"))
            .and(path(route.as_str()))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
    }
    for verb in [TaskVerb::Approve, TaskVerb::Reject, TaskVerb::Retry] {
        client(&server)
            .task_verb(WORKSPACE, "task_1", verb)
            .await
            .expect("task verb");
    }
    let requests = server.received_requests().await.expect("requests");
    let paths: Vec<_> = requests.iter().map(|request| request.url.path()).collect();
    assert_eq!(paths, [approve.as_str(), reject.as_str(), retry.as_str()]);
}

#[tokio::test]
async fn ut_304_prompt_202_decodes_and_optional_fields_are_omitted_or_sent() {
    let route = format!("/api/workspaces/{WORKSPACE}/sessions/{SESSION}/prompt");
    let server = mock_json(&route, 202, support::fixture("prompt_202.json")).await;
    let outcome = client(&server)
        .send_prompt(WORKSPACE, SESSION, &prompt_request())
        .await
        .expect("send prompt");
    assert!(matches!(
        outcome,
        PromptOutcome::Accepted(result) if result.queue_position == Some(2)
    ));

    let mut with_optional = prompt_request();
    with_optional.expected_turn_id = Some("turn-1".to_owned());
    with_optional.runtime = Some(PromptRuntime {
        provider: "claude".to_owned(),
        model: Some("sonnet".to_owned()),
    });
    client(&server)
        .send_prompt(WORKSPACE, SESSION, &with_optional)
        .await
        .expect("send prompt with optional fields");

    let requests = server.received_requests().await.expect("requests");
    let first: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(first["message"], "ship it");
    assert_eq!(first["mode"], "queue");
    assert!(first.get("expected_turn_id").is_none());
    assert!(first.get("runtime").is_none());
    let second: Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(second["expected_turn_id"], "turn-1");
    assert_eq!(
        second["runtime"],
        json!({"provider":"claude","model":"sonnet"})
    );
}

#[tokio::test]
async fn ut_305_prompt_sse_body_is_dropped_without_waiting_for_a_frame() {
    let server = sse_server::SseServer::start(vec![sse_server::ResponseSpec::Hang]).await;
    let result = tokio::time::timeout(
        Duration::from_millis(100),
        Client::tcp(server.address()).send_prompt(WORKSPACE, SESSION, &prompt_request()),
    )
    .await
    .expect("send_prompt read the infinite SSE body")
    .expect("send prompt");
    assert_eq!(result, PromptOutcome::Sent);
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn ut_306_cancel_prompt_posts_an_empty_body() {
    let route = format!("/api/workspaces/{WORKSPACE}/sessions/{SESSION}/prompt/cancel");
    let server = mock_json(&route, 200, "").await;
    client(&server)
        .cancel_prompt(WORKSPACE, SESSION)
        .await
        .expect("cancel prompt");
    let requests = server.received_requests().await.expect("requests");
    assert!(requests[0].body.is_empty());
}

#[tokio::test]
async fn ut_307_post_errors_preserve_status_message_code_and_details() {
    let route = format!("/api/workspaces/{WORKSPACE}/sessions/{SESSION}/prompt");
    let cases = [
        (
            409,
            support::fixture("prompt_409.json"),
            Some("session.active_turn_mismatch"),
        ),
        (
            413,
            support::fixture("prompt_413.json"),
            Some("session.input_queue.full"),
        ),
        (422, r#"{"error":"invalid prompt mode"}"#.to_owned(), None),
        (503, r#"{"error":"daemon is draining"}"#.to_owned(), None),
    ];
    for (status, body, expected_code) in cases {
        let server = mock_json(&route, status, body).await;
        let error = client(&server)
            .send_prompt(WORKSPACE, SESSION, &prompt_request())
            .await
            .expect_err("daemon error");
        match error {
            Error::Daemon {
                status: actual,
                code,
                details,
                ..
            } => {
                assert_eq!(actual, status);
                assert_eq!(code.as_deref(), expected_code);
                if status == 413 {
                    assert_eq!(details.unwrap()["queue_cap"], 10);
                }
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}

#[tokio::test]
async fn ut_308_post_over_uds_sends_host_and_json_body() {
    let server = uds::UdsServer::start(vec![uds::ResponseSpec::json(
        201,
        support::fixture("session_created.json"),
    )])
    .await;
    Client::uds(server.path.clone())
        .create_session(WORKSPACE, "batuta")
        .await
        .expect("create session over UDS");
    let requests = server.requests();
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path_and_query, "/api/sessions");
    assert_eq!(requests[0].host.as_deref(), Some("localhost"));
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[0].body).unwrap(),
        json!({"workspace":WORKSPACE,"agent_name":"batuta"})
    );
}

#[tokio::test]
async fn ut_309_post_400_maps_daemon_and_malformed_2xx_maps_decode() {
    let bad_request = mock_json(
        "/api/sessions",
        400,
        r#"{"error":"invalid create request"}"#,
    )
    .await;
    assert!(matches!(
        client(&bad_request).create_session(WORKSPACE, "missing").await,
        Err(Error::Daemon { status: 400, message, .. }) if message == "invalid create request"
    ));

    let malformed = mock_json("/api/sessions", 201, "not json").await;
    assert!(matches!(
        client(&malformed).create_session(WORKSPACE, "batuta").await,
        Err(Error::Decode {
            context: "create session response",
            ..
        })
    ));
}
