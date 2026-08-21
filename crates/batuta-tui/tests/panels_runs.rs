#[path = "support/panels.rs"]
mod panels_support;
use batuta_tui::{
    Cmd, Msg, Request, StreamId, TimerId,
    app::{Detail, Panel},
    msg::ApiResponse,
    update,
};
use crossterm::event::KeyCode;
use panels_support::*;
use std::time::Duration;

fn load(model: &mut batuta_tui::Model, live: bool) -> Vec<Cmd> {
    respond(
        model,
        runs_request(200),
        ApiResponse::Runs(Box::new(runs_page(live))),
    )
}

#[test]
fn ut_490_rows_glyphs_aggregates_and_query_snapshot() {
    let mut model = model();
    model.focus = Panel::Runs;
    let commands = load(&mut model, true);
    assert!(commands.contains(&Cmd::After(Duration::from_secs(5), TimerId::RunsPoll)));
    insta::assert_snapshot!("runs_populated_120x40", render(&model, 120, 40));
}

#[test]
fn ut_491_star_removes_loop_query() {
    let mut model = model();
    model.focus = Panel::Runs;
    load(&mut model, true);
    let commands = update(&mut model, key(KeyCode::Char('*')));
    assert!(matches!(
        &commands[0],
        Cmd::Get(Request::Runs {
            loop_name: None,
            ..
        })
    ));
    assert!(render(&model, 100, 30).contains("all loops"));
}

#[test]
fn ut_492_filter_matches_id_loop_and_status() {
    let mut model = model();
    model.focus = Panel::Runs;
    load(&mut model, true);
    update(&mut model, key(KeyCode::Char('/')));
    for ch in "implement".chars() {
        update(&mut model, key(KeyCode::Char(ch)));
    }
    assert_eq!(model.runs.items.len(), 1);
    insta::assert_snapshot!("runs_filtered_100x30", render(&model, 100, 30));
}

#[test]
fn ut_493_enter_opens_run_and_stream() {
    let mut model = model();
    model.focus = Panel::Runs;
    load(&mut model, true);
    let commands = update(&mut model, key(KeyCode::Enter));
    assert_eq!(model.focus, Panel::Detail);
    assert!(
        matches!(model.detail, Detail::Run(ref detail) if detail.run_id == "looprun-parent1234")
    );
    assert!(commands.iter().any(
        |cmd| matches!(cmd, Cmd::Get(Request::Run { run, .. }) if run == "looprun-parent1234")
    ));
    assert!(commands.iter().any(|cmd| matches!(cmd, Cmd::StartStream(StreamId::RunEvents(run)) if run == "looprun-parent1234")));
}

#[test]
fn ut_494_empty_state() {
    let mut model = model();
    model.focus = Panel::Runs;
    insta::assert_snapshot!("runs_empty_100x30", render(&model, 100, 30));
}

#[test]
fn ut_495_children_are_indented_under_parent() {
    let mut model = model();
    model.focus = Panel::Runs;
    load(&mut model, true);
    let screen = render(&model, 100, 30);
    let parent = screen
        .lines()
        .find(|line| line.contains("rent1234"))
        .unwrap();
    let child = screen
        .lines()
        .find(|line| line.contains("hild5678"))
        .unwrap();
    assert!(child.find('✓').unwrap() > parent.find('●').unwrap());
}

#[test]
fn ut_496_route_missing_toast() {
    let mut model = model();
    fail(
        &mut model,
        runs_request(201),
        "route missing in this daemon version: GET /api/workspaces/{ws}/loop-runs",
    );
    assert_eq!(
        model.toast.unwrap().text,
        "route missing in this daemon version: GET /api/workspaces/{ws}/loop-runs"
    );
}

#[test]
fn ut_497_live_poll_rearms_independent_of_focus() {
    let mut model = model();
    model.focus = Panel::Attention;
    assert!(
        load(&mut model, true).contains(&Cmd::After(Duration::from_secs(5), TimerId::RunsPoll))
    );
    let commands = update(&mut model, Msg::Timer(TimerId::RunsPoll));
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Get(Request::Runs { .. })))
    );
    assert_eq!(model.focus, Panel::Attention);
}

#[test]
fn ut_012_run_navigation_changes_only_selection() {
    let mut model = model();
    model.focus = Panel::Runs;
    load(&mut model, true);
    let selected = model.runs.selected;
    assert!(update(&mut model, key(KeyCode::Char('j'))).is_empty());
    assert_ne!(model.runs.selected, selected);
    assert_eq!(model.focus, Panel::Runs);
    assert!(matches!(model.detail, Detail::Empty));
}

#[test]
fn ut_498_terminal_result_stops_poll() {
    let mut model = model();
    load(&mut model, true);
    let commands = respond(
        &mut model,
        runs_request(202),
        ApiResponse::Runs(Box::new(runs_page(false))),
    );
    assert!(
        !commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::After(_, TimerId::RunsPoll)))
    );
}

#[test]
fn ut_499_poll_error_keeps_rows_marks_stale_and_rearms() {
    let mut model = model();
    model.focus = Panel::Runs;
    load(&mut model, true);
    let commands = fail(&mut model, runs_request(203), "temporary error");
    assert_eq!(model.runs.items.len(), 2);
    assert!(model.runs_stale);
    assert!(commands.contains(&Cmd::After(Duration::from_secs(5), TimerId::RunsPoll)));
    assert!(render(&model, 100, 30).contains("runs: stale"));
}

#[test]
fn run_navigation_and_refresh_never_post() {
    let mut model = model();
    model.focus = Panel::Runs;
    load(&mut model, true);
    for code in [
        KeyCode::Char('j'),
        KeyCode::Char('k'),
        KeyCode::Enter,
        KeyCode::Char('/'),
        KeyCode::Char('*'),
        KeyCode::Char('r'),
    ] {
        model.focus = Panel::Runs;
        assert!(
            !update(&mut model, key(code))
                .iter()
                .any(|cmd| matches!(cmd, Cmd::Post(_)))
        );
    }
}
