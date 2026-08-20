#[path = "support/panels.rs"]
mod panels_support;

use batuta_tui::{
    ApiResponse, Cmd, Msg, Request, StreamId,
    app::{AppMode, Overlay, Settings},
    update,
};
use compozy_client::{
    RunControl,
    types::{AddWorkspaceOutcome, LoopMutation, Workspace},
};
use crossterm::event::KeyCode;
use panels_support::{fail, key, model, render, respond, runs_page, session_page};

fn workspaces() -> Vec<Workspace> {
    vec![
        Workspace {
            id: "ws-test".into(),
            name: "workspace".into(),
            root_dir: "/tmp/old".into(),
            ..Workspace::default()
        },
        Workspace {
            id: "ws-next".into(),
            name: "next".into(),
            root_dir: "/tmp/next".into(),
            ..Workspace::default()
        },
    ]
}

fn open(model: &mut batuta_tui::Model) -> Request {
    update(model, key(KeyCode::Char('w')))
        .into_iter()
        .find_map(|cmd| match cmd {
            Cmd::Get(request @ Request::Workspaces { .. }) => Some(request),
            _ => None,
        })
        .expect("workspaces get")
}

#[test]
fn ut_620_picker_fetches_and_renders_rows_at_both_sizes() {
    let mut model = model();
    let request = open(&mut model);
    respond(&mut model, request, ApiResponse::Workspaces(workspaces()));
    let screen100 = render(&model, 100, 30);
    let screen120 = render(&model, 120, 40);
    for expected in ["workspace  /tmp/old", "next  /tmp/next"] {
        assert!(screen120.contains(expected), "{screen120}");
    }
    insta::assert_snapshot!("picker_populated_100x30", screen100);
    insta::assert_snapshot!("picker_populated_120x40", screen120);
}

#[test]
fn ut_621_enter_stops_all_clears_pending_and_reboots_new_workspace() {
    let mut model = model();
    model
        .active_streams
        .extend([StreamId::Catalog, StreamId::RunEvents("old".into())]);
    let request = open(&mut model);
    respond(&mut model, request, ApiResponse::Workspaces(workspaces()));
    update(&mut model, key(KeyCode::Char('j')));
    let commands = update(&mut model, key(KeyCode::Enter));
    assert_eq!(
        model.workspace.as_ref().map(|ws| ws.id.as_str()),
        Some("ws-next")
    );
    assert!(model.overlay.is_none());
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::StopStream(StreamId::Catalog)))
    );
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::StopStream(StreamId::RunEvents(id)) if id == "old"))
    );
    assert!(commands.iter().any(
        |cmd| matches!(cmd, Cmd::Get(Request::Sessions { workspace, .. }) if workspace == "ws-next")
    ));
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::StartStream(StreamId::Catalog)))
    );
    assert!(render(&model, 100, 30).contains("ws: next"));
}

#[test]
fn ut_622_escape_cancels_picker_without_workspace_change() {
    let mut model = model();
    let old = model.workspace.clone();
    open(&mut model);
    assert!(update(&mut model, key(KeyCode::Esc)).is_empty());
    assert_eq!(model.workspace, old);
    assert!(model.overlay.is_none());
}

#[test]
fn ut_623_startup_picker_select_reboots_like_manual_switch() {
    let settings = Settings {
        workspace: None,
        ..Settings::default()
    };
    let mut model = batuta_tui::Model::new(settings, AppMode::Full);
    let initial = model.initial_cmds();
    let request = initial
        .into_iter()
        .find_map(|cmd| match cmd {
            Cmd::Get(request @ Request::Workspaces { .. }) => Some(request),
            _ => None,
        })
        .unwrap();
    assert!(matches!(
        model.overlay,
        Some(Overlay::WorkspacePicker { at_start: true, .. })
    ));
    respond(&mut model, request, ApiResponse::Workspaces(workspaces()));
    let commands = update(&mut model, key(KeyCode::Enter));
    assert_eq!(
        model.workspace.as_ref().map(|ws| ws.id.as_str()),
        Some("ws-test")
    );
    assert!(commands.iter().any(
        |cmd| matches!(cmd, Cmd::Get(Request::Sessions { workspace, .. }) if workspace == "ws-test")
    ));
}

#[test]
fn ut_624_empty_picker_message_and_startup_escape_quits() {
    let settings = Settings {
        workspace: None,
        ..Settings::default()
    };
    let mut model = batuta_tui::Model::new(settings, AppMode::Full);
    let request = model
        .initial_cmds()
        .into_iter()
        .find_map(|cmd| match cmd {
            Cmd::Get(request @ Request::Workspaces { .. }) => Some(request),
            _ => None,
        })
        .unwrap();
    respond(&mut model, request, ApiResponse::Workspaces(Vec::new()));
    let screen = render(&model, 100, 30);
    assert!(
        screen.contains("no workspaces — register one with `compozy workspace add`"),
        "{screen}"
    );
    insta::assert_snapshot!("picker_empty_100x30", screen);
    insta::assert_snapshot!("picker_empty_120x40", render(&model, 120, 40));
    assert!(update(&mut model, key(KeyCode::Esc)).contains(&Cmd::Quit));
}

#[test]
fn ut_625_late_post_result_is_ignored_except_success_toast_once() {
    let mut model = model();
    let late = model.allocate(|id| Request::RunControl {
        id,
        workspace: "ws-test".into(),
        run: "old-run".into(),
        control: RunControl::Pause,
    });
    let late_id = late.id();
    let request = open(&mut model);
    respond(&mut model, request, ApiResponse::Workspaces(workspaces()));
    update(&mut model, key(KeyCode::Char('j')));
    update(&mut model, key(KeyCode::Enter));
    let result = Ok(ApiResponse::RunMutation(LoopMutation {
        ok: true,
        run_id: "old-run".into(),
        status: "paused".into(),
    }));
    update(
        &mut model,
        Msg::Api {
            request: late_id,
            result,
        },
    );
    assert_eq!(
        model.workspace.as_ref().map(|ws| ws.id.as_str()),
        Some("ws-next")
    );
    assert_eq!(
        model.toast.as_ref().map(|toast| toast.text.as_str()),
        Some("paused")
    );
    model.toast = None;
    update(
        &mut model,
        Msg::Api {
            request: late_id,
            result: Ok(ApiResponse::RunMutation(LoopMutation {
                ok: true,
                run_id: "old-run".into(),
                status: "paused".into(),
            })),
        },
    );
    assert!(model.toast.is_none());
}

#[test]
fn ut_729_confirmed_onboarding_is_the_only_registration_write_and_boots_after_refetch() {
    let mut model = batuta_tui::Model::new(
        Settings {
            workspace: None,
            ..Settings::default()
        },
        AppMode::Full,
    );
    model.start_workspace_onboarding(batuta_tui::app::WorkspaceCandidate {
        name: "new-workspace".into(),
        root_dir: "/tmp/new-workspace".into(),
    });

    assert!(update(&mut model, key(KeyCode::Char('a'))).is_empty());
    assert!(update(&mut model, key(KeyCode::Esc)).is_empty());
    assert!(model.pending.is_empty());

    update(&mut model, key(KeyCode::Char('a')));
    let add = update(&mut model, key(KeyCode::Enter))
        .into_iter()
        .find_map(|cmd| match cmd {
            Cmd::Post(request @ Request::AddWorkspace { .. }) => Some(request),
            _ => None,
        })
        .expect("confirmation writes once");
    let refetch = update(
        &mut model,
        Msg::Api {
            request: add.id(),
            result: Ok(ApiResponse::WorkspaceAdded(AddWorkspaceOutcome::Added(
                Workspace::default(),
            ))),
        },
    )
    .into_iter()
    .find_map(|cmd| match cmd {
        Cmd::Get(request @ Request::Workspaces { .. }) => Some(request),
        _ => None,
    })
    .expect("successful registration refetches catalog");
    let boot_commands = respond(
        &mut model,
        refetch,
        ApiResponse::Workspaces(vec![Workspace {
            id: "ws-new".into(),
            name: "new-workspace".into(),
            root_dir: "/tmp/new-workspace".into(),
            ..Workspace::default()
        }]),
    );
    let sessions = boot_commands
        .iter()
        .find_map(|cmd| match cmd {
            Cmd::Get(request @ Request::Sessions { .. }) => Some(request.clone()),
            _ => None,
        })
        .expect("normal boot requests sessions");
    let runs = boot_commands
        .iter()
        .find_map(|cmd| match cmd {
            Cmd::Get(request @ Request::Runs { .. }) => Some(request.clone()),
            _ => None,
        })
        .expect("normal boot requests runs");
    let overview_request = boot_commands
        .iter()
        .find_map(|cmd| match cmd {
            Cmd::Get(request @ Request::Overview { .. }) => Some(request.clone()),
            _ => None,
        })
        .expect("normal boot requests overview");
    assert_eq!(
        model
            .workspace
            .as_ref()
            .map(|workspace| workspace.id.as_str()),
        Some("ws-new")
    );
    assert!(matches!(
        model.overlay,
        Some(Overlay::WorkspaceOnboarding { booting: true, .. })
    ));
    respond(
        &mut model,
        sessions,
        ApiResponse::Sessions(Box::new(session_page())),
    );
    assert!(model.overlay.is_some());
    respond(
        &mut model,
        runs,
        ApiResponse::Runs(Box::new(runs_page(false))),
    );
    assert!(model.overlay.is_some());
    let overview = serde_json::from_value(serde_json::json!({
        "attention": {"total": 0, "by_kind": {}, "items": []}
    }))
    .expect("empty overview");
    respond(
        &mut model,
        overview_request,
        ApiResponse::Overview(Box::new(overview)),
    );
    assert!(model.overlay.is_none());
}

#[test]
fn ut_730_onboarding_remains_open_until_normal_boot_succeeds() {
    let mut model = batuta_tui::Model::new(
        Settings {
            workspace: None,
            ..Settings::default()
        },
        AppMode::Full,
    );
    model.start_workspace_onboarding(batuta_tui::app::WorkspaceCandidate {
        name: "new-workspace".into(),
        root_dir: "/tmp/new-workspace".into(),
    });

    update(&mut model, key(KeyCode::Char('a')));
    let add = update(&mut model, key(KeyCode::Enter))
        .into_iter()
        .find_map(|cmd| match cmd {
            Cmd::Post(request @ Request::AddWorkspace { .. }) => Some(request),
            _ => None,
        })
        .expect("confirmation writes once");
    let refetch = update(
        &mut model,
        Msg::Api {
            request: add.id(),
            result: Ok(ApiResponse::WorkspaceAdded(AddWorkspaceOutcome::Added(
                Workspace::default(),
            ))),
        },
    )
    .into_iter()
    .find_map(|cmd| match cmd {
        Cmd::Get(request @ Request::Workspaces { .. }) => Some(request),
        _ => None,
    })
    .expect("successful registration refetches catalog");
    let commands = respond(
        &mut model,
        refetch,
        ApiResponse::Workspaces(vec![Workspace {
            id: "ws-new".into(),
            name: "new-workspace".into(),
            root_dir: "/tmp/new-workspace".into(),
            ..Workspace::default()
        }]),
    );
    let sessions = commands
        .iter()
        .find_map(|cmd| match cmd {
            Cmd::Get(request @ Request::Sessions { .. }) => Some(request.clone()),
            _ => None,
        })
        .expect("normal boot requests sessions");
    assert!(matches!(
        model.overlay,
        Some(Overlay::WorkspaceOnboarding { booting: true, .. })
    ));

    fail(&mut model, sessions, "service unavailable");
    assert!(matches!(
        model.overlay,
        Some(Overlay::WorkspaceOnboarding {
            booting: false,
            message: Some(ref message),
            ..
        }) if message == "workspace selected; startup failed: service unavailable"
    ));
}
