use compozy_client::types::{
    Clarification, ClarifyAnswer, ClarifyResult, Decision, ErrorPayload, LogEvent, LoopEvent,
    LoopRunDetail, LoopRunPage, OverviewResponse, Part, PermissionData, PromptMode, PromptResult,
    Session, SessionPage, SessionResponse, SessionStopped, StatusPayload, Timestamp,
    TranscriptPage, TranscriptSnapshot, WorkspacesResponse,
};
use serde_json::{Value, json};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{FIXTURES}/{name}")).expect("fixture is committed")
}

#[test]
fn ut_340_delivery_two_fixtures_decode() {
    let _: LoopRunPage = serde_json::from_str(&fixture("loop_runs.json")).unwrap();
    let _: LoopRunDetail = serde_json::from_str(&fixture("loop_run.json")).unwrap();
    let _: OverviewResponse = serde_json::from_str(&fixture("overview.json")).unwrap();
    let logs: Value = serde_json::from_str(&fixture("logs.json")).unwrap();
    for event in logs["events"].as_array().unwrap() {
        let _: LogEvent = serde_json::from_value(event.clone()).unwrap();
    }
    let clarifications: Value = serde_json::from_str(&fixture("clarifications.json")).unwrap();
    assert!(clarifications["clarifications"].is_array());
    for name in ["loop_events.sse", "logs_stream.sse", "catalog.sse"] {
        let stream = fixture(name);
        for data in stream
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
        {
            match name {
                "loop_events.sse" => {
                    let _: LoopEvent = serde_json::from_str(data).unwrap();
                }
                "logs_stream.sse" => {
                    let _: LogEvent = serde_json::from_str(data).unwrap();
                }
                _ => {
                    let _: compozy_client::types::CatalogEvent =
                        serde_json::from_str(data).unwrap();
                }
            }
        }
    }
    let _: SessionResponse = serde_json::from_str(&fixture("session_created.json")).unwrap();
    for name in [
        "prompt_409.json",
        "prompt_413.json",
        "approve_409.json",
        "clarify_404.json",
    ] {
        let _: ErrorPayload = serde_json::from_str(&fixture(name)).unwrap();
    }
}

#[test]
fn ut_030_transcript_fixture_decodes_tool_parts() {
    let page: TranscriptPage = serde_json::from_str(&fixture("transcript_page.json")).unwrap();
    let assistant = page
        .entries
        .iter()
        .find(|entry| {
            entry.message.role == compozy_client::types::Role::Assistant
                && entry
                    .message
                    .parts
                    .iter()
                    .any(|part| matches!(part, Part::Text { .. }))
        })
        .unwrap();
    assert!(
        assistant
            .message
            .parts
            .iter()
            .any(|part| matches!(part, Part::Event { .. }))
    );
    assert!(
        assistant
            .message
            .parts
            .iter()
            .any(|part| matches!(part, Part::Text { state: Some(state), .. } if state == "done"))
    );
    assert!(assistant.message.parts.iter().any(|part| matches!(part, Part::Tool { name, state: Some(state), tool_call_id: Some(_), .. } if !name.is_empty() && state == "output-available")));
}

#[test]
fn ut_031_dynamic_tool_decodes() {
    let part: Part =
        serde_json::from_str(r#"{"type":"dynamic-tool","toolName":"x","state":"input-streaming"}"#)
            .unwrap();
    assert!(
        matches!(part, Part::Tool { name, state: Some(state), .. } if name == "x" && state == "input-streaming")
    );
}

#[test]
fn ut_032_tool_prefix_decodes() {
    let part: Part = serde_json::from_str(r#"{"type":"tool-mcp__compozy-hosted-tools__compozy__config_get","state":"output-available"}"#).unwrap();
    assert!(
        matches!(part, Part::Tool { name, .. } if name == "mcp__compozy-hosted-tools__compozy__config_get")
    );
}

#[test]
fn ut_033_events_and_markers_decode() {
    let marker: Part = serde_json::from_str(r#"{"type":"data-compozy-event","data":{"type":"transcript_marker.created","kind":"file_mutation_unverified","summary":"x"}}"#).unwrap();
    assert!(
        matches!(marker, Part::Marker { kind: Some(kind), .. } if kind == "file_mutation_unverified")
    );
    let event: Part =
        serde_json::from_str(r#"{"type":"data-compozy-event","data":{"type":"usage"}}"#).unwrap();
    assert!(matches!(event, Part::Event { .. }));
}

#[test]
fn ut_034_permissions_decode() {
    let part: Part = serde_json::from_str(
        r#"{"type":"data-compozy-permission","data":{"request_id":"req_1","turn_id":"turn-1"}}"#,
    )
    .unwrap();
    assert!(matches!(part, Part::Permission { .. }));
}

#[test]
fn ut_035_reasoning_and_file_decode() {
    let reasoning: Part =
        serde_json::from_str(r#"{"type":"reasoning","text":"x","state":"done"}"#).unwrap();
    assert!(matches!(reasoning, Part::Reasoning { .. }));
    let file: Part =
        serde_json::from_str(r#"{"type":"file","filename":"a.txt","mediaType":"text/plain"}"#)
            .unwrap();
    assert!(
        matches!(file, Part::File { filename: Some(name), media_type: Some(media_type), .. } if name == "a.txt" && media_type == "text/plain")
    );
}

#[test]
fn ut_036_unknown_part_keeps_page_decodable() {
    let page: TranscriptPage = serde_json::from_str(r#"{"entries":[{"message":{"id":"m","role":"assistant","parts":[{"type":"something-new"}]},"start_sequence":1,"sequence":1}]}"#).unwrap();
    assert!(
        matches!(page.entries[0].message.parts[0], Part::Unknown { ref type_ } if type_ == "something-new")
    );
}

#[test]
fn ut_037_unknown_fields_null_and_timestamps_are_tolerated() {
    let session: Session =
        serde_json::from_str(r#"{"id":"s","archived_at":null,"new_field":{"nested":true}}"#)
            .unwrap();
    assert!(session.archived_at.is_none());
    for value in ["2026-08-16T11:16:13Z", "2026-08-17T17:52:09.588764927Z"] {
        assert!(
            Timestamp::from(value.to_owned()).parsed().is_some(),
            "{value}"
        );
    }
}

#[test]
fn captured_fixtures_decode_to_their_contract_types() {
    let _: StatusPayload = serde_json::from_str(&fixture("status.json")).unwrap();
    let _: WorkspacesResponse = serde_json::from_str(&fixture("workspaces.json")).unwrap();
    let _: SessionPage = serde_json::from_str(&fixture("sessions.json")).unwrap();
    let _: SessionResponse = serde_json::from_str(&fixture("session.json")).unwrap();
    let _: ErrorPayload = serde_json::from_str(&fixture("error_404.json")).unwrap();
    let _: ErrorPayload = serde_json::from_str(&fixture("error_503_draining.json")).unwrap();
    let stopped = fixture("session_stopped.sse");
    let stopped_data = stopped
        .lines()
        .find_map(|line| line.strip_prefix("data: "))
        .unwrap();
    let _: SessionStopped = serde_json::from_str(stopped_data).unwrap();
    for name in ["stream_normal.sse", "stream_reset.sse"] {
        let stream = fixture(name);
        let data = stream
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap();
        let _: TranscriptSnapshot = serde_json::from_str(data).unwrap();
    }
}

#[test]
fn ut_341_permission_data_parses_the_permission_part_payload() {
    let part: Part = serde_json::from_value(json!({
        "type": "data-compozy-permission",
        "data": {
            "request_id": "req_3f9c",
            "turn_id": "turn-073fb634a25a1f32",
            "title": "Bash",
            "action": "rm -rf build/",
            "decision": null,
            "raw": {
                "tool_input": {"command": "rm -rf build/"},
                "options": [{
                    "decision": "allow-once",
                    "option_id": "allow-once",
                    "kind": "allow",
                    "label": "Allow once"
                }]
            }
        }
    }))
    .unwrap();
    let permission = PermissionData::from_part(&part)
        .expect("permission part")
        .expect("permission data");
    assert_eq!(permission.request_id, "req_3f9c");
    assert_eq!(permission.turn_id, "turn-073fb634a25a1f32");
    assert_eq!(permission.raw.tool_input["command"], "rm -rf build/");
    assert_eq!(permission.raw.options[0].decision, "allow-once");
}

#[test]
fn ut_342_session_activity_turn_fields_and_badge_decode() {
    let response: SessionResponse = serde_json::from_str(&fixture("session.json")).unwrap();
    assert_eq!(response.session.badge.as_deref(), Some("running"));
    let activity = response.session.activity.expect("activity");
    assert_eq!(activity.turn_id.as_deref(), Some("turn-e4d3bc17589f5666"));
    assert!(activity.turn_started_at.is_some());
}

#[test]
fn ut_343_clarify_answer_serializes_exactly_one_key() {
    for (answer, expected) in [
        (ClarifyAnswer::Choice(2), json!({"choice_index": 2})),
        (ClarifyAnswer::Text("x".to_owned()), json!({"text": "x"})),
    ] {
        let value = serde_json::to_value(answer).unwrap();
        assert_eq!(value, expected);
        assert_eq!(value.as_object().unwrap().len(), 1);
    }
}

#[test]
fn ut_344_decision_and_prompt_mode_wire_strings_round_trip() {
    for (decision, wire) in [
        (Decision::AllowOnce, "allow-once"),
        (Decision::AllowAlways, "allow-always"),
        (Decision::RejectOnce, "reject-once"),
        (Decision::RejectAlways, "reject-always"),
    ] {
        assert_eq!(serde_json::to_value(decision).unwrap(), wire);
        assert_eq!(
            serde_json::from_value::<Decision>(Value::String(wire.to_owned())).unwrap(),
            decision
        );
    }
    for (mode, wire) in [
        (PromptMode::Queue, "queue"),
        (PromptMode::Steer, "steer"),
        (PromptMode::Interrupt, "interrupt"),
    ] {
        assert_eq!(serde_json::to_value(mode).unwrap(), wire);
        assert_eq!(
            serde_json::from_value::<PromptMode>(Value::String(wire.to_owned())).unwrap(),
            mode
        );
    }
}

#[test]
fn hand_authored_write_fixtures_decode_to_contract_types() {
    let prompt: Value = serde_json::from_str(&fixture("prompt_202.json")).unwrap();
    let _: PromptResult = serde_json::from_value(prompt["prompt"].clone()).unwrap();
    for name in [
        "prompt_409.json",
        "prompt_413.json",
        "approve_409.json",
        "clarify_404.json",
    ] {
        let _: ErrorPayload = serde_json::from_str(&fixture(name)).unwrap();
    }
    let approve: Value = serde_json::from_str(&fixture("approve_200.json")).unwrap();
    assert_eq!(approve["status"], "approved");
    let _: ClarifyResult = serde_json::from_str(&fixture("clarify_200.json")).unwrap();
    let _: SessionResponse = serde_json::from_str(&fixture("session_created.json")).unwrap();

    let clarification: Clarification = serde_json::from_value(json!({
        "request_id":"req_1",
        "session_id":"sess_1",
        "agent_name":"code_implementer",
        "question":"Which environment?",
        "choices":[]
    }))
    .unwrap();
    assert_eq!(clarification.request_id, "req_1");
}
