use batuta_tui::{
    app::{AppMode, Model, SessionHeader},
    cmd::{Cmd, Request, StreamId},
};
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
