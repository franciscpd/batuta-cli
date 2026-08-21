use batuta_tui::{
    app::{Model, SessionHeader},
    cmd::Cmd,
    msg::Msg,
    update,
};
use compozy_client::types::{Entry, Part, Role, TranscriptPage, UiMessage};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn model() -> Model {
    Model::tail(SessionHeader {
        workspace: "workspace".into(),
        workspace_id: "ws-test".into(),
        session_id: "sess-test".into(),
        agent: "batuta".into(),
        name: Some("task".into()),
        state: "active".into(),
        warning: None,
    })
}
fn press(code: KeyCode) -> Msg {
    Msg::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn tool_entry(start_sequence: i64) -> Entry {
    Entry {
        start_sequence,
        sequence: start_sequence,
        message: UiMessage {
            id: format!("message-{start_sequence}"),
            role: Role::Assistant,
            metadata: None,
            parts: vec![Part::Tool {
                name: "tool".into(),
                tool_call_id: Some(format!("tool-{start_sequence}")),
                state: Some("output-available".into()),
                input: None,
                output: None,
                error_text: None,
                title: None,
            }],
        },
    }
}

#[test]
fn ut_680_tail_keys_remain_safe() {
    let mut model = model();
    assert!(update(&mut model, press(KeyCode::Esc)).is_empty());
    let commands = update(&mut model, press(KeyCode::Char('q')));
    assert!(matches!(commands.last(), Some(Cmd::Quit)));
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Cmd::Post(_)))
    );
}
#[test]
fn delivery_one_dirty_tick_is_preserved() {
    let mut model = model();
    model.dirty = false;
    assert!(update(&mut model, Msg::Tick).is_empty());
    model.dirty = true;
    assert_eq!(update(&mut model, Msg::Tick), vec![Cmd::Render]);
    assert!(update(&mut model, Msg::Tick).is_empty());
}

#[test]
fn ut_713_debug_toggle_preserves_selected_source_entry() {
    let mut model = model();
    batuta_tui::app::page_into_detail(
        model.session_detail_mut().unwrap(),
        TranscriptPage {
            entries: (1..=6).map(tool_entry).collect(),
            epoch: 1,
            generation: 1,
            max_sequence: 6,
            has_older: false,
            limit: 200,
            ..TranscriptPage::default()
        },
    );
    model.session_detail_mut().unwrap().view.raw_debug = true;
    model.session_detail_mut().unwrap().view.selection = 5;

    update(&mut model, press(KeyCode::Char('D')));
    let detail = model.session_detail().unwrap();
    assert!(!detail.view.raw_debug);
    assert_eq!(detail.view.selection, 0);

    update(&mut model, press(KeyCode::Char('D')));
    let detail = model.session_detail().unwrap();
    assert!(detail.view.raw_debug);
    assert_eq!(detail.view.selection, 5);
    assert_eq!(
        detail.transcript.entries()[detail.view.selection].start_sequence,
        6
    );
}
