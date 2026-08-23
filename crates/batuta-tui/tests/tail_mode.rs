use batuta_tui::{
    Msg,
    app::{AppMode, Model, SessionHeader},
    cmd::{Cmd, Request, StreamId},
    update,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn tail_model() -> Model {
    Model::tail(SessionHeader {
        workspace: "w".into(),
        workspace_id: "ws".into(),
        session_id: "sess".into(),
        agent: "batuta".into(),
        name: None,
        state: "active".into(),
        warning: None,
    })
}

fn press(model: &mut Model, code: KeyCode) -> Vec<Cmd> {
    update(model, Msg::Key(KeyEvent::new(code, KeyModifiers::NONE)))
}

#[test]
fn ut_667_tail_initial_commands_only_transcript() {
    let mut model = Model::tail(SessionHeader {
        workspace: "w".into(),
        workspace_id: "ws".into(),
        session_id: "sess".into(),
        agent: "batuta".into(),
        name: None,
        state: "active".into(),
        warning: None,
    });
    assert_eq!(model.mode, AppMode::TailOnly);
    let commands = model.initial_cmds();
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Cmd::Get(Request::TranscriptPage { .. })))
    );
    assert!(!commands.iter().any(|command| matches!(
        command,
        Cmd::Get(Request::Sessions { .. } | Request::Runs { .. } | Request::Overview { .. })
            | Cmd::StartStream(StreamId::Catalog)
    )));
}

#[test]
fn ut_783_tail_only_search_prompt_stays_inert_and_quit_still_works() {
    // `tail` mode never renders `session::footer` (`views/mod.rs`
    // early-returns `tail::view`), so `/` opening `search.focused = true`
    // is invisible: no prompt appears, yet `text_field_focused()` then
    // swallows every keystroke, including `q`, silently breaking quit.
    // `/`, `n`, `N`, and `y` must stay inert in `TailOnly`, mirroring the
    // `tail_only_inert` gate for panel-focus actions.
    let mut model = tail_model();
    press(&mut model, KeyCode::Char('/'));
    assert!(
        model
            .session_detail()
            .is_some_and(|detail| detail.view.search.is_none()),
        "`/` must not open a search prompt in TailOnly mode"
    );
    assert!(!model.text_field_focused());

    let commands = press(&mut model, KeyCode::Char('q'));
    assert!(
        commands.iter().any(|cmd| matches!(cmd, Cmd::Quit)),
        "q must still be able to quit in TailOnly mode: {commands:?}"
    );
}
