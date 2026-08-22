#[path = "support/panels.rs"]
mod panels_support;

use batuta_tui::{
    Cmd, Msg,
    app::{Overlay, Panel},
    update,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use panels_support::{model, render};

fn model_with_workspace() -> batuta_tui::Model {
    // `panels_support::model()` already threads `Settings::default()`
    // (which carries `workspace: Some(ws-test)`) into `Model::new`; this
    // wrapper just names that precondition for the palette tests below,
    // several of which dispatch actions (e.g. `session: new`) that no-op
    // without a workspace.
    let model = model();
    assert!(model.workspace.is_some(), "fixture must carry a workspace");
    model
}

fn press_ctrl(model: &mut batuta_tui::Model, c: char) {
    update(
        model,
        Msg::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)),
    );
}

fn press_key(model: &mut batuta_tui::Model, code: KeyCode) -> Vec<Cmd> {
    update(model, Msg::Key(KeyEvent::new(code, KeyModifiers::NONE)))
}

fn type_str(model: &mut batuta_tui::Model, s: &str) {
    for c in s.chars() {
        press_key(model, KeyCode::Char(c));
    }
}

#[test]
fn palette_opens_filters_and_dispatches() {
    let mut model = model_with_workspace();
    press_ctrl(&mut model, 'p');
    assert!(matches!(model.overlay, Some(Overlay::Palette { .. })));
    type_str(&mut model, "quit");
    press_key(&mut model, KeyCode::Enter);
    // quit is guarded: first Enter arms the guard exactly like pressing q.
    assert!(model.overlay.is_none());
}

#[test]
fn palette_esc_closes_without_dispatch() {
    let mut model = model_with_workspace();
    press_ctrl(&mut model, 'p');
    press_key(&mut model, KeyCode::Esc);
    assert!(model.overlay.is_none());
    assert_eq!(model.focus, Panel::Sessions);
}

#[test]
fn palette_query_filters_catalog_entries() {
    let mut model = model_with_workspace();
    press_ctrl(&mut model, 'p');
    type_str(&mut model, "focus");
    let screen = render(&model, 100, 30);
    for expected in [
        "focus: sessions",
        "focus: deliver runs",
        "focus: attention",
        "focus: detail",
    ] {
        assert!(screen.contains(expected), "{screen}");
    }
    assert!(!screen.contains("workspace: switch"), "{screen}");
    assert!(!screen.contains("quit"), "{screen}");
}

#[test]
fn palette_new_session_focuses_sessions_then_dispatches() {
    let mut model = model_with_workspace();
    model.focus = Panel::Runs;
    press_ctrl(&mut model, 'p');
    type_str(&mut model, "session: new");
    let commands = press_key(&mut model, KeyCode::Enter);
    assert!(model.overlay.is_none());
    assert_eq!(model.focus, Panel::Sessions);
    assert!(commands.iter().any(|cmd| matches!(cmd, Cmd::Post(_))));
}
