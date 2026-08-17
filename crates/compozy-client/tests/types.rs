use compozy_client::types::{
    ErrorPayload, Part, Session, SessionPage, SessionResponse, SessionStopped, StatusPayload,
    Timestamp, TranscriptPage, TranscriptSnapshot, WorkspacesResponse,
};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{FIXTURES}/{name}")).expect("fixture is committed")
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
