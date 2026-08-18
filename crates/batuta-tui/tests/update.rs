use batuta_tui::{
    app::{Model, SessionHeader},
    cmd::Cmd,
    msg::Msg,
    update,
};
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
