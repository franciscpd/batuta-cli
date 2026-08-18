use batuta_tui::{
    app::{FooterState, Model, SessionHeader, update},
    keymap,
    msg::Msg,
    theme::Theme,
    views,
};
use compozy_client::types::{Entry, Part, Role, TranscriptPage, UiMessage};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use serde_json::json;

fn entry(start: i64, role: Role, parts: Vec<Part>) -> Entry {
    Entry {
        start_sequence: start,
        sequence: start + 1,
        message: UiMessage {
            id: format!("m-{start}"),
            role,
            metadata: None,
            parts,
        },
    }
}

fn model() -> Model {
    let mut model = Model::new(SessionHeader {
        workspace: "batuta-cli".into(),
        workspace_id: "ws_e619d7250e618324".into(),
        session_id: "sess-807cee9774b33f68".into(),
        agent: "batuta".into(),
        name: Some("Analisar e implementar projeto conforme documentação existente".into()),
        state: "active".into(),
        warning: None,
    });
    let entries = vec![
        entry(
            10,
            Role::User,
            vec![Part::Text {
                text: "Nos vamos analisar para implementar o projeto".into(),
                state: Some("done".into()),
            }],
        ),
        entry(
            20,
            Role::Assistant,
            vec![
                Part::Text {
                    text: "Vou começar pelo **gate** de entrega.\n\n- ler contrato\n- implementar"
                        .into(),
                    state: Some("streaming".into()),
                },
                Part::Reasoning {
                    text: (1..=14)
                        .map(|n| format!("reason {n}"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    state: Some("done".into()),
                },
                Part::Tool {
                    name: "mcp__compozy-hosted-tools__compozy__config_get".into(),
                    tool_call_id: Some("tool-1".into()),
                    state: Some("output-available".into()),
                    input: Some(json!({"path":"loops.inputs.batuta-deliver.auto_commit"})),
                    output: Some(json!({"value":true})),
                    error_text: None,
                    title: None,
                },
                Part::Tool {
                    name: "mcp__compozy-hosted-tools__compozy__provider_models_list".into(),
                    tool_call_id: Some("tool-error".into()),
                    state: Some("output-error".into()),
                    input: None,
                    output: None,
                    error_text: Some("output schema mismatch".into()),
                    title: None,
                },
                Part::Tool {
                    name: "Task".into(),
                    tool_call_id: Some("tool-2".into()),
                    state: Some("input-available".into()),
                    input: Some(json!({"description":"Track A: daemon API facts"})),
                    output: None,
                    error_text: None,
                    title: None,
                },
                Part::Marker {
                    kind: Some("prompt_accepted".into()),
                    summary: Some("turn accepted".into()),
                    occurred_at: None,
                    evidence: None,
                },
                Part::Marker {
                    kind: Some("file_mutation_unverified".into()),
                    summary: Some("executor edited files without verification".into()),
                    occurred_at: None,
                    evidence: None,
                },
                Part::Permission {
                    data: json!({"request_id":"req_3f9c", "turn_id":"turn-073fb634a25a1f32", "summary":"Bash: rm -rf build/"}),
                },
                Part::File {
                    filename: Some("report.txt".into()),
                    media_type: Some("text/plain".into()),
                    url: None,
                },
                Part::Unknown {
                    type_: "future-part".into(),
                },
            ],
        ),
    ];
    update(
        &mut model,
        Msg::Page(Ok(TranscriptPage {
            entries,
            epoch: 1,
            generation: 1,
            max_sequence: 30,
            has_older: false,
            limit: 200,
            ..TranscriptPage::default()
        })),
    );
    model
}

fn render(mut model: Model, width: u16, height: u16) -> String {
    update(&mut model, Msg::Resize(width, height));
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| views::view(&model, frame))
        .expect("draw");
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            let mut line = String::new();
            for x in 0..width {
                line.push_str(buffer[(x, y)].symbol());
            }
            line.trim_end().to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn ut_120_core_screen_snapshot() {
    insta::assert_snapshot!(render(model(), 100, 30));
}

#[test]
fn ut_122_responsive_snapshots() {
    insta::assert_snapshot!("screen_80x24", render(model(), 80, 24));
    insta::assert_snapshot!("screen_120x40", render(model(), 120, 40));
    insta::assert_snapshot!("screen_200x60", render(model(), 200, 60));
}

#[test]
fn ut_125_no_color_keeps_glyph_hierarchy() {
    let mut model = model();
    model.theme = Theme::new(false);
    update(&mut model, Msg::Resize(80, 24));
    assert!(
        model
            .render_cache
            .values()
            .flat_map(|text| &text.lines)
            .flat_map(|line| &line.spans)
            .all(|span| span.style.fg.is_none() && span.style.bg.is_none())
    );
    let screen = render(model, 80, 24);
    assert!(screen.contains("✓"));
    assert!(screen.contains("! file_mutation_unverified"));
}

#[test]
fn ut_121_expanded_tool_shows_pretty_input_and_output() {
    let mut model = model();
    update(
        &mut model,
        Msg::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
    );
    let screen = render(model, 100, 30);
    assert!(screen.contains("input"));
    assert!(screen.contains("output"));
    assert!(screen.contains("\"value\": true"));
}

#[test]
fn ut_123_reasoning_toggle_reveals_lines() {
    let mut model = model();
    update(
        &mut model,
        Msg::Key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE)),
    );
    assert!(render(model, 100, 30).contains("reason 14"));
}

#[test]
fn ut_124_and_ut_128_markdown_and_unclosed_fence_do_not_panic() {
    let mut model = Model::new(SessionHeader {
        workspace: "w".into(),
        workspace_id: "ws".into(),
        session_id: "sess-md".into(),
        agent: "batuta".into(),
        name: None,
        state: "active".into(),
        warning: None,
    });
    update(
        &mut model,
        Msg::Page(Ok(TranscriptPage {
            entries: vec![entry(
                1,
                Role::Assistant,
                vec![Part::Text {
                    text: "# Heading\n\n- item\n\n```rust\nfn main() {".into(),
                    state: Some("done".into()),
                }],
            )],
            max_sequence: 2,
            limit: 200,
            ..TranscriptPage::default()
        })),
    );
    let screen = render(model, 80, 24);
    assert!(screen.contains("Heading"));
    assert!(screen.contains("fn main()"));
}

#[test]
fn ut_129_streaming_text_has_cursor() {
    assert!(render(model(), 100, 30).contains("implementar▍"));
}

#[test]
fn ut_130_string_tool_payload_is_verbatim() {
    let mut model = Model::new(SessionHeader {
        workspace: "w".into(),
        workspace_id: "ws".into(),
        session_id: "sess-tool".into(),
        agent: "batuta".into(),
        name: None,
        state: "active".into(),
        warning: None,
    });
    update(
        &mut model,
        Msg::Page(Ok(TranscriptPage {
            entries: vec![entry(
                1,
                Role::Assistant,
                vec![Part::Tool {
                    name: "x".into(),
                    tool_call_id: None,
                    state: Some("output-available".into()),
                    input: None,
                    output: Some(json!("plain string")),
                    error_text: None,
                    title: None,
                }],
            )],
            max_sequence: 2,
            limit: 200,
            ..TranscriptPage::default()
        })),
    );
    update(
        &mut model,
        Msg::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
    );
    assert!(render(model, 80, 24).contains("plain string"));
}

#[test]
fn ut_121_expanded_payload_is_capped_at_two_hundred_lines() {
    let payload = (0..201)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = Model::new(SessionHeader {
        workspace: "w".into(),
        workspace_id: "ws".into(),
        session_id: "sess-tool".into(),
        agent: "batuta".into(),
        name: None,
        state: "active".into(),
        warning: None,
    });
    update(
        &mut model,
        Msg::Page(Ok(TranscriptPage {
            entries: vec![entry(
                1,
                Role::Assistant,
                vec![Part::Tool {
                    name: "x".into(),
                    tool_call_id: None,
                    state: Some("output-error".into()),
                    input: None,
                    output: None,
                    error_text: Some(payload),
                    title: None,
                }],
            )],
            max_sequence: 2,
            limit: 200,
            ..TranscriptPage::default()
        })),
    );
    update(
        &mut model,
        Msg::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
    );
    assert!(render(model, 80, 240).contains("… truncated"));
}

#[test]
fn ut_131_oversized_part_is_replaced_by_size_notice() {
    let text = "x".repeat(1024 * 1024 + 1);
    let mut model = Model::new(SessionHeader {
        workspace: "w".into(),
        workspace_id: "ws".into(),
        session_id: "sess-large".into(),
        agent: "batuta".into(),
        name: None,
        state: "active".into(),
        warning: None,
    });
    update(
        &mut model,
        Msg::Page(Ok(TranscriptPage {
            entries: vec![entry(
                1,
                Role::Assistant,
                vec![Part::Text { text, state: None }],
            )],
            max_sequence: 2,
            limit: 200,
            ..TranscriptPage::default()
        })),
    );
    assert!(render(model, 80, 10).contains("[part too large: 1048577 bytes]"));
}

#[test]
fn ut_134_permission_decision_replaces_card() {
    let mut model = Model::new(SessionHeader {
        workspace: "w".into(),
        workspace_id: "ws".into(),
        session_id: "sess-perm".into(),
        agent: "batuta".into(),
        name: None,
        state: "active".into(),
        warning: None,
    });
    update(
        &mut model,
        Msg::Page(Ok(TranscriptPage {
            entries: vec![entry(
                1,
                Role::Assistant,
                vec![Part::Permission {
                    data: json!({"request_id":"r", "turn_id":"t", "summary":"read"}),
                }],
            )],
            epoch: 1,
            generation: 1,
            max_sequence: 2,
            limit: 200,
            ..TranscriptPage::default()
        })),
    );
    let mut decided = entry(
        1,
        Role::Assistant,
        vec![Part::Permission {
            data: json!({"request_id":"r", "turn_id":"t", "summary":"read", "decision":"approved"}),
        }],
    );
    decided.sequence = 3;
    update(
        &mut model,
        Msg::Stream(compozy_client::TranscriptEvent::Delta(
            compozy_client::types::TranscriptDelta {
                epoch: 1,
                generation: 1,
                entries: vec![decided],
                cursor: 3,
                max_sequence: 3,
                ..Default::default()
            },
        )),
    );
    assert!(render(model, 80, 12).contains("approved"));
}

#[test]
fn ut_126_footer_variants_use_exact_copy() {
    let variants = [
        FooterState::Stopped {
            reason: None,
            detail: None,
        },
        FooterState::Reconnecting(3),
        FooterState::Offline,
        FooterState::Resynchronized("epoch_mismatch".into()),
        FooterState::NewBelow(3),
    ];
    let rendered = variants
        .into_iter()
        .map(|footer| {
            let mut value = model();
            value.footer = footer;
            render(value, 100, 8)
        })
        .collect::<Vec<_>>()
        .join("\n---\n");
    insta::assert_snapshot!(rendered);
}

#[test]
fn ut_127_resync_and_beginning_lines_render_inline() {
    let mut model = model();
    update(
        &mut model,
        Msg::Stream(compozy_client::TranscriptEvent::Snapshot(
            compozy_client::types::TranscriptSnapshot {
                entries: vec![entry(40, Role::Assistant, vec![])],
                epoch: 2,
                generation: 2,
                max_sequence: 40,
                reset: true,
                reason: Some("epoch_mismatch".into()),
                ..Default::default()
            },
        )),
    );
    let screen = render(model, 80, 12);
    assert!(screen.contains("─ resynchronized (epoch_mismatch) ─"));
    assert!(screen.contains("beginning of transcript"));
}

#[test]
fn ut_177_visible_window_bounds_layout_work_for_ten_thousand_entries() {
    let mut model = Model::new(SessionHeader {
        workspace: "w".into(),
        workspace_id: "ws".into(),
        session_id: "sess-many".into(),
        agent: "batuta".into(),
        name: None,
        state: "active".into(),
        warning: None,
    });
    let entries = (0..10_000)
        .map(|value| entry(value, Role::Assistant, Vec::new()))
        .collect();
    update(
        &mut model,
        Msg::Page(Ok(TranscriptPage {
            entries,
            max_sequence: 10_000,
            limit: 10_000,
            ..Default::default()
        })),
    );
    let range = views::transcript::visible_entry_range(&model, 20);
    assert!(range.len() <= 22);
}

#[test]
fn ut_182_stopped_footer_includes_reason_and_detail() {
    let mut model = model();
    model.footer = FooterState::Stopped {
        reason: Some("user_request".into()),
        detail: Some("finished".into()),
    };
    let status = views::footer::status(&model);
    assert!(status.contains("user_request"));
    assert!(status.contains("finished"));
}

#[test]
fn ut_132_and_ut_133_narrow_and_empty_states() {
    assert!(render(model(), 30, 12).contains("▸ batuta"));
    assert!(render(model(), 19, 8).contains("terminal too narrow"));
    let empty = Model::new(SessionHeader {
        workspace: "w".into(),
        workspace_id: "ws".into(),
        session_id: "sess-empty".into(),
        agent: "batuta".into(),
        name: None,
        state: "active".into(),
        warning: None,
    });
    let screen = render(empty, 80, 8);
    assert!(screen.contains("batuta · w · sess-empty"));
    assert!(screen.contains("no transcript yet"));
}

#[test]
fn ut_143_keymap_and_footer_are_generated_from_one_table() {
    assert_eq!(keymap::footer(), keymap::FOOTER);
    for binding in keymap::BINDINGS {
        assert!(keymap::FOOTER.contains(binding.keys));
        assert!(keymap::FOOTER.contains(binding.action));
    }
}
