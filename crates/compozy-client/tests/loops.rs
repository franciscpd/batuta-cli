mod support;

use compozy_client::{Client, Error, GateDecision, LoopRunQuery, RunControl};
use serde_json::Value;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_json, method, path, query_param},
};

const WS: &str = "ws_e619d7250e618324";
const RUN: &str = "looprun-1c4ef728a0628179";

fn client(server: &MockServer) -> Client {
    Client::tcp(server.address().to_string())
}

#[tokio::test]
async fn ut_310_loop_runs_query_and_aggregates() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/workspaces/{WS}/loop-runs")))
        .and(query_param("loop", "batuta-deliver"))
        .and(query_param("limit", "50"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(support::fixture("loop_runs.json")),
        )
        .mount(&server)
        .await;
    let page = client(&server)
        .loop_runs(
            WS,
            &LoopRunQuery {
                loop_: Some("batuta-deliver"),
                status: None,
                limit: 50,
            },
        )
        .await
        .unwrap();
    assert!(!page.runs.is_empty());
    assert_eq!(page.aggregates.total, page.runs.len() as u64);
}

#[tokio::test]
async fn ut_311_loop_run_decodes_generation_outputs() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/workspaces/{WS}/loop-runs/{RUN}")))
        .respond_with(ResponseTemplate::new(200).set_body_string(support::fixture("loop_run.json")))
        .mount(&server)
        .await;
    let detail = client(&server).loop_run(WS, RUN).await.unwrap();
    let output = detail
        .generations
        .iter()
        .flat_map(|generation| &generation.outputs)
        .find(|output| output.resolved_runtime.is_some())
        .expect("captured output with resolved runtime");
    assert!(!output.node_id.is_empty());
    assert!(!output.status.is_empty());
    assert!(output.resolved_runtime.is_some());
}

#[tokio::test]
async fn ut_313_controls_and_gate_use_exact_routes_and_bodies() {
    let server = MockServer::start().await;
    for (verb, status) in [
        ("pause", "paused"),
        ("resume", "running"),
        ("cancel", "canceled"),
        ("kill", "killed"),
    ] {
        Mock::given(method("POST"))
            .and(path(format!("/api/workspaces/{WS}/loop-runs/{RUN}/{verb}")))
            .and(body_json(serde_json::json!({})))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"ok":true,"run_id":RUN,"status":status})),
            )
            .mount(&server)
            .await;
    }
    for control in [
        RunControl::Pause,
        RunControl::Resume,
        RunControl::Cancel,
        RunControl::Kill,
    ] {
        assert!(
            client(&server)
                .loop_run_control(WS, RUN, control)
                .await
                .unwrap()
                .ok
        );
    }
    Mock::given(method("POST"))
        .and(path(format!(
            "/api/workspaces/{WS}/loop-runs/{RUN}/approve"
        )))
        .and(body_json(
            serde_json::json!({"gate_id":"gate-1","decision":"approve"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok":true})))
        .mount(&server)
        .await;
    client(&server)
        .loop_run_approve(WS, RUN, "gate-1", GateDecision::Approve)
        .await
        .unwrap();
}

#[tokio::test]
async fn ut_314_control_errors_preserve_status_and_message() {
    for status in [404, 409, 422] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(status)
                    .set_body_json(serde_json::json!({"error":"bad transition"})),
            )
            .mount(&server)
            .await;
        assert!(
            matches!(client(&server).loop_run_control(WS, RUN, RunControl::Pause).await, Err(Error::Daemon { status: actual, message, .. }) if actual == status && message == "bad transition")
        );
    }
}

#[test]
fn loop_fixture_contains_expected_runtime_shape() {
    let value: Value = serde_json::from_str(&support::fixture("loop_run.json")).unwrap();
    assert!(value["generations"][0]["outputs"].as_array().is_some());
}
