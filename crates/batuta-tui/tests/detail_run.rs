#[path = "support/panels.rs"]
mod panels_support;

use batuta_tui::{
    AnyStreamEvent, ApiResponse, Cmd, Msg, Request, StreamId,
    app::{Detail, Panel, RunDetail, StreamStatus},
    update,
};
use compozy_client::{
    GateDecision, RunControl, StreamEvent,
    types::{LoopEvent, LoopRunDetail},
};
use crossterm::event::KeyCode;
use panels_support::{fail, key, model, render, respond};

fn run_detail(generations: usize, status: &str) -> LoopRunDetail {
    let generation_count = generations;
    let generations = (1..=generations)
        .map(|generation| serde_json::json!({
            "generation": generation,
            "origin": if generation == 1 { "initial" } else { "retry" },
            "outputs": [{
                "node_id": format!("implement-{generation}"),
                "item_index": (generation - 1) as i64,
                "status": if generation == generations { status } else { "failed" },
                "attempt": generation as i64,
                "child_loop_run_id": if generation == generations { "looprun-child" } else { "" },
                "session_id": if generation == generations { "sess-node" } else { "" },
                "resolved_runtime": {"provider":"claude","model":"sonnet"},
                "failure_class": if generation == generations { "" } else { "transient" }
            }]
        }))
        .collect::<Vec<_>>();
    serde_json::from_value(serde_json::json!({
        "run": {
            "id":"looprun-parent", "workspace_id":"ws-test", "loop_name":"batuta-deliver",
            "status":status, "generation":generation_count as i64, "created_at":"2026-08-18T20:00:00Z",
            "started_at":"2026-08-18T20:05:52Z", "last_progress_at":"2026-08-18T22:46:28Z",
            "parent_loop_run_id":"looprun-root", "inputs":{"slug":"mvp-tui","auto_commit":true}
        },
        "generations": generations
    })).unwrap()
}

fn event(seq: i64, kind: &str, payload: serde_json::Value) -> LoopEvent {
    serde_json::from_value(serde_json::json!({
        "id": format!("evt-{seq}"), "seq":seq, "kind":kind,
        "payload":payload, "at":"2026-08-18T20:05:52Z"
    }))
    .unwrap()
}

fn loaded(generations: usize, status: &str) -> batuta_tui::Model {
    let mut model = model();
    model.focus = Panel::Detail;
    model.detail = Detail::Run(Box::new(RunDetail {
        run: Some(run_detail(generations, status)),
        run_id: "looprun-parent".into(),
        events: Vec::new(),
        selection: 0,
        confirm: None,
        stream: StreamStatus::Live,
    }));
    model
        .active_streams
        .insert(StreamId::RunEvents("looprun-parent".into()));
    model
}

fn send(model: &mut batuta_tui::Model, loop_event: LoopEvent) -> Vec<Cmd> {
    update(
        model,
        Msg::Stream {
            id: StreamId::RunEvents("looprun-parent".into()),
            event: AnyStreamEvent::Run(StreamEvent::Event(loop_event)),
        },
    )
}

#[test]
fn ut_590_run_payload_renders_header_inputs_nodes_child_and_runtime() {
    let model = loaded(1, "running");
    let screen = render(&model, 120, 40);
    for expected in [
        "looprun-parent",
        "batuta-deliver",
        "running",
        "generation 1",
        "slug=mvp-tui",
        "implement-1",
        "looprun-child",
        "claude/sonnet",
    ] {
        assert!(screen.contains(expected), "missing {expected:?}\n{screen}");
    }
    insta::assert_snapshot!("run_detail_1_generation_120x40", screen);
    insta::assert_snapshot!("run_detail_1_generation_100x30", render(&model, 100, 30));
}

#[test]
fn ut_591_events_append_as_typed_lines_and_unknown_is_dimmed() {
    let mut model = loaded(1, "running");
    for item in [
        event(1, "node_running", serde_json::json!({"node_id":"load"})),
        event(2, "node_succeeded", serde_json::json!({"node_id":"load"})),
        event(3, "status_changed", serde_json::json!({"status":"running"})),
        event(4, "generation_started", serde_json::json!({"generation":2})),
        event(
            5,
            "needs_approval",
            serde_json::json!({"gate_id":"gate-ab12","revision":2}),
        ),
        event(6, "future_event", serde_json::json!({"value":"kept"})),
    ] {
        send(&mut model, item);
    }
    let screen = render(&model, 120, 40);
    for expected in [
        "node_running  load",
        "node_succeeded  load",
        "status_changed  running",
        "generation_started  gen 2",
        "gate gate-ab12 (rev 2)",
        "future_event",
    ] {
        assert!(screen.contains(expected), "missing {expected:?}\n{screen}");
    }
    insta::assert_snapshot!("run_detail_events_120x40", screen);
    insta::assert_snapshot!("run_detail_events_100x30", render(&model, 100, 30));
}

#[test]
fn ut_592_terminal_status_event_stops_stream() {
    let mut model = loaded(1, "running");
    let commands = send(
        &mut model,
        event(7, "status_changed", serde_json::json!({"status":"done"})),
    );
    assert_eq!(
        commands,
        vec![Cmd::StopStream(StreamId::RunEvents(
            "looprun-parent".into()
        ))]
    );
    assert!(
        !model
            .active_streams
            .contains(&StreamId::RunEvents("looprun-parent".into()))
    );
}

#[test]
fn ut_593_enter_opens_child_run_then_node_session() {
    let mut model = loaded(1, "running");
    let child = update(&mut model, key(KeyCode::Enter));
    assert!(
        child
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Get(Request::Run { run, .. }) if run == "looprun-child"))
    );
    model = loaded(1, "running");
    update(&mut model, key(KeyCode::Char('j')));
    let session = update(&mut model, key(KeyCode::Enter));
    assert!(session.iter().any(
        |cmd| matches!(cmd, Cmd::Get(Request::Session { session, .. }) if session == "sess-node")
    ));
}

#[test]
fn ut_594_selected_gate_renders_verbs() {
    let mut model = loaded(1, "running");
    send(
        &mut model,
        event(
            9,
            "needs_approval",
            serde_json::json!({"gate_id":"gate-ab12","revision":2}),
        ),
    );
    update(&mut model, key(KeyCode::Char('G')));
    let screen = render(&model, 100, 30);
    assert!(
        screen.contains("? gate gate-ab12 (rev 2)  a approve · x reject"),
        "{screen}"
    );
    insta::assert_snapshot!("run_detail_gate_100x30", screen);
    insta::assert_snapshot!("run_detail_gate_120x40", render(&model, 120, 40));
}

#[test]
fn ut_595_last_event_sequence_is_kept_as_reconnect_cursor() {
    let mut model = loaded(1, "running");
    send(
        &mut model,
        event(42, "node_running", serde_json::json!({"node_id":"x"})),
    );
    assert_eq!(
        model
            .stream_cursors
            .get(&StreamId::RunEvents("looprun-parent".into()))
            .map(String::as_str),
        Some("42")
    );
}

#[test]
fn ut_596_footer_has_all_run_actions() {
    let screen = render(&loaded(1, "running"), 100, 30);
    assert!(
        screen.contains("p pause  u resume  k kill  Enter open  a/x gate  L logs"),
        "{screen}"
    );
}

#[test]
fn ut_597_run_404_toasts_and_clears_detail() {
    let mut model = loaded(1, "running");
    let request = Request::Run {
        id: batuta_tui::RequestId(90),
        workspace: "ws-test".into(),
        run: "looprun-parent".into(),
    };
    fail(&mut model, request, "HTTP 404: run not found (not_found)");
    assert!(matches!(model.detail, Detail::Empty));
    assert!(
        model
            .toast
            .as_ref()
            .is_some_and(|toast| toast.text.contains("run not found"))
    );
}

#[test]
fn ut_598_three_generations_render_with_newest_expanded() {
    let screen = render(&loaded(3, "running"), 120, 40);
    for generation in 1..=3 {
        assert!(
            screen.contains(&format!("generation {generation}")),
            "{screen}"
        );
    }
    assert!(screen.contains("implement-3"), "{screen}");
    insta::assert_snapshot!("run_detail_3_generations_120x40", screen);
    insta::assert_snapshot!(
        "run_detail_3_generations_100x30",
        render(&loaded(3, "running"), 100, 30)
    );
}

#[test]
fn ut_599_duplicate_event_sequence_is_suppressed() {
    let mut model = loaded(1, "running");
    send(
        &mut model,
        event(12, "node_running", serde_json::json!({"node_id":"once"})),
    );
    send(
        &mut model,
        event(
            12,
            "node_running",
            serde_json::json!({"node_id":"duplicate"}),
        ),
    );
    let Detail::Run(detail) = &model.detail else {
        panic!()
    };
    assert_eq!(detail.events.len(), 1);
}

fn post(model: &mut batuta_tui::Model, key_code: KeyCode) -> Request {
    update(model, key(key_code))
        .into_iter()
        .find_map(|cmd| match cmd {
            Cmd::Post(request) => Some(request),
            _ => None,
        })
        .expect("post")
}

#[test]
fn ut_600_pause_resume_and_confirmed_kill_post_controls() {
    for (key_code, expected) in [('p', RunControl::Pause), ('u', RunControl::Resume)] {
        let request = post(&mut loaded(1, "running"), KeyCode::Char(key_code));
        assert!(matches!(request, Request::RunControl { control, .. } if control == expected));
    }
    let mut model = loaded(1, "running");
    assert!(update(&mut model, key(KeyCode::Char('k'))).is_empty());
    let Detail::Run(detail) = &model.detail else {
        panic!()
    };
    assert_eq!(
        detail
            .confirm
            .as_ref()
            .map(|confirm| confirm.prompt.as_str()),
        Some("kill run looprun-parent? Enter/Esc")
    );
    assert!(matches!(
        post(&mut model, KeyCode::Enter),
        Request::RunControl {
            control: RunControl::Kill,
            ..
        }
    ));
}

#[test]
fn ut_601_selected_gate_posts_approve_and_reject() {
    for (key_code, decision) in [('a', GateDecision::Approve), ('x', GateDecision::Reject)] {
        let mut model = loaded(1, "running");
        send(
            &mut model,
            event(
                20,
                "needs_approval",
                serde_json::json!({"gate_id":"gate-x","revision":3}),
            ),
        );
        update(&mut model, key(KeyCode::Char('G')));
        assert!(
            matches!(post(&mut model, KeyCode::Char(key_code)), Request::RunApprove { gate_id, decision:got, .. } if gate_id == "gate-x" && got == decision)
        );
    }
}

#[test]
fn run_kill_confirmation_rechecks_draining_before_dispatch() {
    let mut model = loaded(1, "running");
    assert!(update(&mut model, key(KeyCode::Char('k'))).is_empty());

    model.daemon.status = "draining".into();
    let commands = update(&mut model, key(KeyCode::Enter));

    assert!(commands.iter().all(|cmd| !matches!(cmd, Cmd::Post(_))));
    assert_eq!(
        model.toast.as_ref().map(|toast| toast.text.as_str()),
        Some("can't kill run — daemon draining, try again once it recovers")
    );
    let Detail::Run(detail) = &model.detail else {
        panic!()
    };
    assert_eq!(detail.confirm, None);
}

#[test]
fn run_gate_decisions_are_refused_while_draining() {
    for (key_code, verb) in [('a', "approve"), ('x', "reject")] {
        let mut model = loaded(1, "running");
        send(
            &mut model,
            event(
                20,
                "needs_approval",
                serde_json::json!({"gate_id":"gate-x","revision":3}),
            ),
        );
        update(&mut model, key(KeyCode::Char('G')));
        model.daemon.status = "draining".into();

        let commands = update(&mut model, key(KeyCode::Char(key_code)));

        assert!(commands.iter().all(|cmd| !matches!(cmd, Cmd::Post(_))));
        let expected = format!("can't {verb} — daemon draining, try again once it recovers");
        assert_eq!(
            model.toast.as_ref().map(|toast| toast.text.as_str()),
            Some(expected.as_str())
        );
    }
}

#[test]
fn ut_602_control_results_toast_and_refetch_both_surfaces() {
    for (control, toast) in [
        (RunControl::Pause, "paused"),
        (RunControl::Resume, "resumed"),
        (RunControl::Kill, "kill requested"),
    ] {
        let mut model = loaded(1, "running");
        let key_code = if control == RunControl::Pause {
            KeyCode::Char('p')
        } else if control == RunControl::Resume {
            KeyCode::Char('u')
        } else {
            update(&mut model, key(KeyCode::Char('k')));
            KeyCode::Enter
        };
        let request = post(&mut model, key_code);
        let commands = respond(
            &mut model,
            request,
            ApiResponse::RunMutation(
                serde_json::from_value(
                    serde_json::json!({"ok":true,"run_id":"looprun-parent","status":"running"}),
                )
                .unwrap(),
            ),
        );
        assert_eq!(
            model.toast.as_ref().map(|item| item.text.as_str()),
            Some(toast)
        );
        assert!(
            commands
                .iter()
                .any(|cmd| matches!(cmd, Cmd::Get(Request::Runs { .. })))
        );
        assert!(
            commands
                .iter()
                .any(|cmd| matches!(cmd, Cmd::Get(Request::Run { .. })))
        );
    }
}

#[test]
fn ut_603_gate_key_without_selected_gate_hints_and_does_not_post() {
    let mut model = loaded(1, "running");
    assert!(
        !update(&mut model, key(KeyCode::Char('a')))
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Post(_)))
    );
    assert!(
        model
            .toast
            .as_ref()
            .is_some_and(|toast| toast.text.contains("select a gate"))
    );
}

#[test]
fn ut_604_escape_cancels_kill_confirmation() {
    let mut model = loaded(1, "running");
    update(&mut model, key(KeyCode::Char('k')));
    assert!(
        update(&mut model, key(KeyCode::Esc))
            .iter()
            .all(|cmd| !matches!(cmd, Cmd::Post(_)))
    );
    let Detail::Run(detail) = &model.detail else {
        panic!()
    };
    assert_eq!(detail.confirm, None);
}

#[test]
fn ut_605_invalid_transition_is_sticky_and_keeps_code() {
    let mut model = loaded(1, "running");
    let request = post(&mut model, KeyCode::Char('p'));
    fail(
        &mut model,
        request,
        "HTTP 422: cannot pause done run (invalid_transition)",
    );
    assert!(model.toast.as_ref().is_some_and(
        |toast| toast.sticky && toast.text == "cannot pause done run (invalid_transition)"
    ));
}

#[test]
fn ut_606_terminal_run_control_is_blocked() {
    let mut model = loaded(1, "done");
    assert!(
        !update(&mut model, key(KeyCode::Char('p')))
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Post(_)))
    );
    assert_eq!(
        model.toast.as_ref().map(|toast| toast.text.as_str()),
        Some("run is already done")
    );
}

#[test]
fn ut_607_gate_denial_preserves_daemon_code() {
    let mut model = loaded(1, "running");
    send(
        &mut model,
        event(
            30,
            "needs_approval",
            serde_json::json!({"gate_id":"gate-x","revision":1}),
        ),
    );
    update(&mut model, key(KeyCode::Char('G')));
    let request = post(&mut model, KeyCode::Char('a'));
    fail(
        &mut model,
        request,
        "HTTP 409: gate revision changed (gate_revision_mismatch)",
    );
    assert_eq!(
        model.toast.as_ref().map(|toast| toast.text.as_str()),
        Some("gate revision changed (gate_revision_mismatch)")
    );
}
