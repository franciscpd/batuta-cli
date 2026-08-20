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
fn model_with(entries: Vec<Entry>) -> Model {
    let mut model = Model::tail(SessionHeader {
        workspace: "batuta-cli".into(),
        workspace_id: "ws_e619d7250e618324".into(),
        session_id: "sess-807cee9774b33f68".into(),
        agent: "batuta".into(),
        name: Some("Analisar e implementar projeto conforme documentação existente".into()),
        state: "active".into(),
        warning: None,
    });
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

fn tool_fixture() -> Model {
    model_with(vec![
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
    ])
}

fn fixture(name: &str) -> Model {
    match name {
        "empty" => model_with(Vec::new()),
        "short" => model_with(vec![
            entry(10, Role::User, vec![Part::Text { text: "Need a release checklist".into(), state: None }]),
            entry(20, Role::Assistant, vec![Part::Text { text: "I will verify the release checklist.".into(), state: Some("done".into()) }]),
        ]),
        "long" => model_with(vec![
            entry(10, Role::User, vec![Part::Text { text: "Unicode: café 東京\n  indented continuation\nlong-token-0123456789abcdef0123456789abcdef0123456789abcdef".into(), state: None }]),
            entry(20, Role::Assistant, vec![Part::Text { text: "Markdown **preserves** whitespace and wraps this deliberately extended conversation without dropping content.".into(), state: Some("streaming".into()) }]),
        ]),
        "tool" => tool_fixture(),
        "error" => model_with(vec![entry(10, Role::Assistant, vec![
            Part::Text { text: "The deployment could not be completed. Retry after checking the token.".into(), state: None },
            Part::Tool { name: "deploy".into(), tool_call_id: Some("failed-deploy".into()), state: Some("output-error".into()), input: Some(json!({"path":"/absolute/private/deploy.json"})), output: Some(json!({"diagnostic":"remote returned 403"})), error_text: Some("HTTP 403: authorization failed".into()), title: None },
        ])]),
        "attention" => model_with(vec![entry(10, Role::Assistant, vec![
            Part::Text { text: "A confirmation is required before deleting the cache.".into(), state: None },
            Part::Permission { data: json!({"request_id":"req-confirm", "turn_id":"turn-confirm", "title":"Delete the build cache", "raw":{"tool_input":{"path":"/tmp/cache"}, "options":[{"decision":"allow-always"}]}}) },
        ])]),
        _ => unreachable!("unknown fixture"),
    }
}
fn render_buffer(
    mut model: Model,
    width: u16,
    height: u16,
) -> (String, Vec<ratatui::buffer::Cell>) {
    update(&mut model, Msg::Resize(width, height));
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| views::view(&model, frame)).unwrap();
    let buffer = terminal.backend().buffer();
    let text = (0..height)
        .map(|y| {
            let mut line = String::new();
            for x in 0..width {
                line.push_str(buffer[(x, y)].symbol());
            }
            line.trim_end().to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n");
    (text, buffer.content.to_vec())
}

fn render(model: Model, width: u16, height: u16) -> String {
    render_buffer(model, width, height).0
}

#[test]
fn ut_709_adjacent_operational_updates_are_losslessly_grouped() {
    let entries = (1..=6)
        .map(|sequence| {
            entry(
                sequence,
                Role::Assistant,
                vec![Part::Tool {
                    name: format!("status-{sequence}"),
                    tool_call_id: None,
                    state: Some("completed".into()),
                    input: None,
                    output: None,
                    error_text: None,
                    title: None,
                }],
            )
        })
        .collect();
    let collapsed = model_with(entries);
    assert!(render(collapsed, 120, 40).contains("▶ 6 tool updates · completed  Enter expand"));

    let entries = (1..=6)
        .map(|sequence| {
            entry(
                sequence,
                Role::Assistant,
                vec![Part::Tool {
                    name: format!("status-{sequence}"),
                    tool_call_id: None,
                    state: Some("completed".into()),
                    input: None,
                    output: None,
                    error_text: None,
                    title: None,
                }],
            )
        })
        .collect();
    let mut expanded = model_with(entries);
    expanded
        .session_detail_mut()
        .unwrap()
        .view
        .expanded
        .insert(1);
    let expanded = render(expanded, 120, 40);
    for sequence in 1..=6 {
        assert!(expanded.contains(&format!("status-{sequence}")));
    }
}

#[test]
fn ut_732_canonical_render_matrix() {
    for (fixture_name, required_text) in [
        ("empty", "no transcript yet"),
        ("short", "release checklist"),
        ("long", "Markdown"),
        ("tool", "file_mutation_unverified"),
        ("error", "deployment could not be completed"),
        ("attention", "approval"),
    ] {
        for (theme_name, color, variant) in [
            ("dark", true, ThemeVariant::Dark),
            ("light", true, ThemeVariant::Light),
            ("no_color", false, ThemeVariant::Light),
        ] {
            for (width, height) in [(90, 30), (120, 40), (180, 50)] {
                let mut model = fixture(fixture_name);
                model.theme = Theme::with_variant(color, variant, None);
                let (output, cells) = render_buffer(model, width, height);
                assert!(
                    output.contains(required_text),
                    "{fixture_name} at {width}x{height} lost required content"
                );
                if color {
                    assert!(
                        cells
                            .iter()
                            .all(|cell| ansi_16_or_reset(cell.fg) && ansi_16_or_reset(cell.bg)),
                        "{fixture_name} emitted non-ANSI color"
                    );
                } else {
                    assert!(
                        cells
                            .iter()
                            .all(|cell| cell.fg == Color::Reset && cell.bg == Color::Reset),
                        "NO_COLOR emitted foreground/background styling"
                    );
                }
                insta::assert_snapshot!(
                    format!("ut_732_{fixture_name}_{theme_name}_{width}x{height}"),
                    output
                );
            }
        }
    }
}

fn ansi_16_or_reset(color: Color) -> bool {
    matches!(
        color,
        Color::Reset
            | Color::Black
            | Color::Red
            | Color::Green
            | Color::Yellow
            | Color::Blue
            | Color::Magenta
            | Color::Cyan
            | Color::Gray
            | Color::DarkGray
            | Color::LightRed
            | Color::LightGreen
            | Color::LightYellow
            | Color::LightBlue
            | Color::LightMagenta
            | Color::LightCyan
            | Color::White
    )
}

#[test]
fn ut_707_ut_708_tool_and_error_disclosure_are_reversible_and_human_first() {
    let collapsed = render(tool_fixture(), 120, 40);
    assert!(collapsed.contains("▶ tool"));
    assert!(!collapsed.contains("loops.inputs.batuta-deliver.auto_commit"));
    let mut expanded_model = tool_fixture();
    expanded_model
        .session_detail_mut()
        .unwrap()
        .view
        .expanded
        .insert(20);
    let expanded = render(expanded_model, 120, 40);
    assert!(expanded.contains("loops.inputs.batuta-deliver.auto_commit"));
    insta::assert_snapshot!("ut_732_tool_collapsed", collapsed);
    insta::assert_snapshot!("ut_732_tool_expanded", expanded);

    let error = render(fixture("error"), 120, 40);
    assert!(
        error.find("deployment could not be completed").unwrap() < error.find("× failed").unwrap()
    );
}
#[test]
fn ut_680_delivery_one_tail_layout() {
    let output = render(tool_fixture(), 80, 24);
    assert!(output.contains("batuta · batuta-cli"));
    assert!(output.contains("file_mutation_unverified"));
    assert!(output.contains("j/k move"));
}

#[test]
fn e2e_700_semantic_terminal_journey() {
    let mut dark_model = tool_fixture();
    dark_model.theme = Theme::with_variant(true, ThemeVariant::Dark, None);
    let dark_text = render(dark_model, 120, 40);
    let mut light_model = tool_fixture();
    light_model.theme = Theme::with_variant(true, ThemeVariant::Light, None);
    assert_eq!(render(light_model, 120, 40), dark_text);
    assert!(dark_text.contains("✓ completed"));
    assert!(dark_text.contains("× failed"));
    assert!(dark_text.contains("▶ tool"));

    let mut no_color_model = tool_fixture();
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
