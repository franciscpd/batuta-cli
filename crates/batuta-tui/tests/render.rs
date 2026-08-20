use batuta_tui::{
    app::{Model, SessionHeader, page_into_detail},
    msg::Msg,
    theme::{Theme, ThemeVariant},
    update, views,
};
use compozy_client::types::{Entry, Part, Role, TranscriptPage, UiMessage};
use ratatui::{Terminal, backend::TestBackend, style::Color};
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
    let mut model = Model::tail(SessionHeader {
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
                    data: json!({"request_id":"req_3f9c","turn_id":"turn-073fb634a25a1f32","summary":"Bash: rm -rf build/"}),
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
    page_into_detail(
        model.session_detail_mut().unwrap(),
        TranscriptPage {
            entries,
            epoch: 1,
            generation: 1,
            max_sequence: 30,
            has_older: false,
            limit: 200,
            ..Default::default()
        },
    );
    model
}
fn render(mut model: Model, width: u16, height: u16) -> String {
    update(&mut model, Msg::Resize(width, height));
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| views::view(&model, frame)).unwrap();
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
fn delivery_one_snapshots_are_byte_for_byte() {
    insta::assert_snapshot!("screen_80x24", render(model(), 80, 24));
    insta::assert_snapshot!("screen_120x40", render(model(), 120, 40));
    insta::assert_snapshot!("screen_200x60", render(model(), 200, 60));
}
#[test]
fn ut_680_delivery_one_tail_layout() {
    let output = render(model(), 80, 24);
    assert!(output.contains("batuta · batuta-cli"));
    assert!(output.contains("file_mutation_unverified"));
    assert!(output.contains("j/k move"));
}

#[test]
fn e2e_700_semantic_terminal_journey() {
    let mut dark_model = model();
    dark_model.theme = Theme::with_variant(true, ThemeVariant::Dark, None);
    let dark_text = render(dark_model, 120, 40);
    let mut light_model = model();
    light_model.theme = Theme::with_variant(true, ThemeVariant::Light, None);
    assert_eq!(render(light_model, 120, 40), dark_text);
    assert!(dark_text.contains("✓ completed"));
    assert!(dark_text.contains("× failed"));
    assert!(dark_text.contains("▶ tool"));

    let mut no_color_model = model();
    no_color_model.theme = Theme::with_variant(false, ThemeVariant::Light, None);
    update(&mut no_color_model, Msg::Resize(120, 40));
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| views::view(&no_color_model, frame))
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert!(buffer.content.iter().all(|cell| cell.fg == Color::Reset));
    assert!(buffer.content.iter().all(|cell| cell.bg == Color::Reset));
    assert_eq!(render(no_color_model, 120, 40), dark_text);
}
