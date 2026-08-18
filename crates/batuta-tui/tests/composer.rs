#[path = "support/panels.rs"]
mod panels_support;

use batuta_tui::{
    Cmd, Msg,
    app::{Detail, Panel, SessionDetail},
    composer::{Composer, TextareaComposer},
    msg::AnyStreamEvent,
    update,
};
use compozy_client::{
    TranscriptEvent,
    types::{Entry, Role, Session, TranscriptDelta, UiMessage},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use panels_support::{key, key_with, model, render};

fn detail_model() -> batuta_tui::Model {
    let mut model = model();
    model.detail = Detail::Session(Box::new(SessionDetail::new(Session {
        id: "sess-a".into(),
        agent_name: "batuta".into(),
        state: "active".into(),
        ..Default::default()
    })));
    model.focus = Panel::Detail;
    model
}

#[test]
fn ut_560_composer_focus_edit_newline_clear_and_escape() {
    let mut model = detail_model();
    update(&mut model, key(KeyCode::Char('i')));
    update(&mut model, key(KeyCode::Char('h')));
    update(&mut model, key_with(KeyCode::Enter, KeyModifiers::ALT));
    update(&mut model, key(KeyCode::Char('i')));
    assert_eq!(model.session_detail().unwrap().composer.text(), "h\ni");
    update(
        &mut model,
        key_with(KeyCode::Char('u'), KeyModifiers::CONTROL),
    );
    assert!(model.session_detail().unwrap().composer.is_empty());
    update(&mut model, key(KeyCode::Esc));
    assert!(!model.session_detail().unwrap().composer.focused);
    update(&mut model, key(KeyCode::Enter));
    assert!(model.session_detail().unwrap().composer.focused);
}

#[test]
fn composer_trait_uses_textarea_for_real_editing() {
    let mut composer = TextareaComposer::default();
    composer.insert(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    composer.insert(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    composer.insert(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));
    assert_eq!(composer.text(), "ba");
    composer.clear();
    assert!(composer.is_empty());
}

#[test]
fn ut_566_composer_is_three_lines_and_soft_wraps_at_100x30() {
    let mut model = detail_model();
    model.session_detail_mut().unwrap().composer.set_text(
        "a long draft that must soft wrap inside the three line composer without escaping its panel",
    );
    insta::assert_snapshot!("session_composer_long_100x30", render(&model, 100, 30));
}

#[test]
fn composer_long_draft_snapshot_120x40() {
    let mut model = detail_model();
    model.session_detail_mut().unwrap().composer.set_text(
        "a long draft that must soft wrap inside the three line composer without escaping its panel",
    );
    insta::assert_snapshot!("session_composer_long_120x40", render(&model, 120, 40));
}

#[test]
fn ut_567_empty_draft_enter_does_not_post() {
    let mut model = detail_model();
    model.session_detail_mut().unwrap().composer.focused = true;
    assert!(update(&mut model, key(KeyCode::Enter)).is_empty());
}

#[test]
fn ut_568_draft_over_64_kib_is_rejected() {
    let mut model = detail_model();
    model.session_detail_mut().unwrap().composer.focused = true;
    model
        .session_detail_mut()
        .unwrap()
        .composer
        .set_text("x".repeat(65_537));
    let commands = update(&mut model, key(KeyCode::Enter));
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Cmd::Post(_)))
    );
    assert_eq!(model.toast.unwrap().text, "prompt too long (max 64 KiB)");
}

#[test]
fn ut_569_delta_does_not_disturb_composer_or_cursor() {
    let mut model = detail_model();
    let detail = model.session_detail_mut().unwrap();
    detail.composer.focused = true;
    detail.composer.set_text("draft");
    detail
        .composer
        .insert(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    let cursor = detail.composer.cursor();
    let delta = TranscriptDelta {
        epoch: 1,
        generation: 1,
        entries: vec![Entry {
            message: UiMessage {
                id: "m1".into(),
                role: Role::Assistant,
                parts: vec![],
                ..Default::default()
            },
            start_sequence: 1,
            sequence: 1,
        }],
        cursor: 1,
        max_sequence: 1,
        ..Default::default()
    };
    update(
        &mut model,
        Msg::Stream {
            id: batuta_tui::StreamId::Transcript("sess-a".into()),
            event: AnyStreamEvent::Transcript(TranscriptEvent::Delta(delta)),
        },
    );
    let composer = &model.session_detail().unwrap().composer;
    assert_eq!(composer.text(), "draft");
    assert_eq!(composer.cursor(), cursor);
}
