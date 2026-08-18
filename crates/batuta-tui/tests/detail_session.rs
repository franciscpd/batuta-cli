#[path = "support/panels.rs"]
mod panels_support;

use batuta_tui::{
    Cmd, Msg, Request, StreamId, TimerId,
    app::{Detail, FooterState, Panel, SessionDetail, StreamStatus},
    msg::{AnyStreamEvent, ApiResponse},
    update,
};
use compozy_client::{
    RunControl, StreamEvent, TranscriptEvent,
    types::{ApproveRequest, CatalogEvent, ClarifyAnswer, Decision, Session, TranscriptSnapshot},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use panels_support::{fail, key, model, render, respond, session_page, sessions_request};
use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color, Modifier},
};

fn detail_model(state: &str, busy: bool) -> batuta_tui::Model {
    let mut model = model();
    model.detail = Detail::Session(Box::new(SessionDetail::new(Session {
        id: "sess-a".into(),
        agent_name: "batuta".into(),
        state: state.into(),
        badge: busy.then(|| "running".into()),
        activity: busy
            .then(|| serde_json::from_value(serde_json::json!({"turn_id":"turn-1"})).unwrap()),
        ..Default::default()
    })));
    model.focus = Panel::Detail;
    model
        .active_streams
        .insert(StreamId::Transcript("sess-a".into()));
    model
}

#[test]
fn ut_550_session_detail_renders_delivery_one_transcript_inside_panel() {
    let mut model = detail_model("active", false);
    let page = serde_json::from_str(include_str!(
        "../../compozy-client/tests/fixtures/transcript_page.json"
    ))
    .unwrap();
    batuta_tui::app::page_into_detail(model.session_detail_mut().unwrap(), page);
    update(&mut model, Msg::Tick);
    insta::assert_snapshot!("session_detail_transcript_100x30", render(&model, 100, 30));
}

#[test]
fn ut_551_selected_inline_permission_renders_all_verbs() {
    let mut model = detail_model("active", false);
    let page = serde_json::from_value(serde_json::json!({
        "entries":[{"message":{"id":"m","role":"assistant","parts":[{
            "type":"data-compozy-permission",
            "data":{"request_id":"req-card","turn_id":"turn-1","title":"Bash","raw":{
                "tool_input":{"command":"rm -rf build/"},
                "options":[
                    {"decision":"allow-once"},{"decision":"allow-always"},
                    {"decision":"reject-once"},{"decision":"reject-always"}
                ]
            }}
        }]} ,"start_sequence":1,"sequence":1}],
        "epoch":1,"generation":1,"max_sequence":1
    }))
    .unwrap();
    batuta_tui::app::page_into_detail(model.session_detail_mut().unwrap(), page);
    update(&mut model, Msg::Tick);
    let screen = render(&model, 120, 40);
    assert!(screen.contains("a allow once") && screen.contains("X reject always"));
}

#[test]
fn ut_552_file_mutation_unverified_is_highlighted() {
    let mut model = detail_model("active", false);
    let page = serde_json::from_value(serde_json::json!({
        "entries":[{"message":{"id":"m","role":"assistant","parts":[{"type":"data-compozy-event","data":{"type":"transcript_marker.file_mutation_unverified","kind":"file_mutation_unverified","summary":"could not verify write"}}]},"start_sequence":1,"sequence":1}],
        "epoch":1,"generation":1,"max_sequence":1
    })).unwrap();
    batuta_tui::app::page_into_detail(model.session_detail_mut().unwrap(), page);
    update(&mut model, Msg::Tick);
    insta::assert_snapshot!(
        "session_file_mutation_warning_100x30",
        render(&model, 100, 30)
    );
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| batuta_tui::views::view(&model, frame))
        .unwrap();
    let marker = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .find(|cell| cell.symbol() == "!")
        .expect("mutation warning marker");
    assert_eq!(marker.fg, Color::Yellow);
    assert!(marker.modifier.contains(Modifier::BOLD));
}

#[test]
fn ut_553_footer_states_have_exact_copy() {
    let cases = [
        (FooterState::Live, "live"),
        (
            FooterState::Stopped {
                reason: None,
                detail: None,
            },
            "stopped — sending a prompt restarts it",
        ),
        (
            FooterState::Reconnecting(3),
            "stream lost — reconnecting (attempt 3)",
        ),
        (FooterState::Offline, "daemon offline — last state shown"),
        (
            FooterState::Resynchronized("epoch_mismatch".into()),
            "resynchronized (epoch_mismatch)",
        ),
        (FooterState::NewBelow(3), "3 new below — G to jump"),
    ];
    for (state, text) in cases {
        let mut model = detail_model("active", false);
        model.session_detail_mut().unwrap().view.footer = state;
        assert!(render(&model, 100, 30).contains(text));
    }
}

#[test]
fn ut_554_switching_sessions_stops_old_starts_new_and_drops_state() {
    let mut model = detail_model("active", false);
    model
        .sessions
        .set_items(session_page().sessions.iter().map(Into::into).collect());
    model.sessions.selected = Some(1);
    model.focus = Panel::Sessions;
    let commands = update(&mut model, key(KeyCode::Enter));
    assert!(commands.contains(&Cmd::StopStream(StreamId::Transcript("sess-a".into()))));
    assert!(commands.contains(&Cmd::StartStream(StreamId::Transcript("sess-b".into()))));
    assert_eq!(model.session_detail().unwrap().session.id, "sess-b");
    assert!(model.session_detail().unwrap().transcript.is_empty());
}

#[test]
fn ut_555_selection_changes_debounce_to_last_session() {
    let mut model = model();
    respond(
        &mut model,
        sessions_request(99),
        ApiResponse::Sessions(Box::new(session_page())),
    );
    for _ in 0..5 {
        update(&mut model, key(KeyCode::Char('j')));
    }
    let armed = update(&mut model, Msg::Timer(TimerId::DetailSwitchDebounce));
    assert!(
        armed
            .iter()
            .any(|cmd| matches!(cmd, Cmd::StartStream(StreamId::Transcript(id)) if id == "sess-b"))
    );
}

#[test]
fn ut_580_stopped_session_send_restarts_stream_after_success() {
    let mut model = detail_model("stopped", false);
    model.session_detail_mut().unwrap().view.stopped = true;
    model.session_detail_mut().unwrap().composer.focused = true;
    model
        .session_detail_mut()
        .unwrap()
        .composer
        .set_text("restart");
    let request = update(&mut model, key(KeyCode::Enter))
        .into_iter()
        .find_map(|cmd| match cmd {
            Cmd::Post(request) => Some(request),
            _ => None,
        })
        .unwrap();
    let commands = respond(
        &mut model,
        request,
        ApiResponse::Prompt(compozy_client::types::PromptOutcome::Sent),
    );
    assert!(commands.contains(&Cmd::StartStream(StreamId::Transcript("sess-a".into()))));
}

#[test]
fn ut_581_stopped_event_stops_without_reconnect() {
    let mut model = detail_model("active", false);
    let stopped = serde_json::from_value(serde_json::json!({"stop_reason":"done"})).unwrap();
    let commands = update(
        &mut model,
        Msg::Stream {
            id: StreamId::Transcript("sess-a".into()),
            event: AnyStreamEvent::Transcript(TranscriptEvent::Stopped(stopped)),
        },
    );
    assert_eq!(
        commands,
        vec![Cmd::StopStream(StreamId::Transcript("sess-a".into()))]
    );
    assert!(!commands.iter().any(|cmd| matches!(cmd, Cmd::After(_, _))));
}

#[test]
fn ut_582_catalog_upsert_active_reopens_stopped_detail_stream() {
    let mut model = detail_model("stopped", false);
    model.session_detail_mut().unwrap().stream = StreamStatus::Stopped;
    model.active_streams.insert(StreamId::Catalog);
    let event = CatalogEvent {
        kind: "upserted".into(),
        workspace_id: "ws-test".into(),
        session_id: "sess-a".into(),
    };
    let commands = update(
        &mut model,
        Msg::Stream {
            id: StreamId::Catalog,
            event: AnyStreamEvent::Catalog(StreamEvent::Event(event)),
        },
    );
    assert!(commands.contains(&Cmd::After(
        std::time::Duration::from_millis(300),
        TimerId::CatalogDebounce
    )));
    let requests = update(&mut model, Msg::Timer(TimerId::CatalogDebounce));
    let request = requests
        .into_iter()
        .find_map(|command| match command {
            Cmd::Get(request @ Request::Sessions { .. }) => Some(request),
            _ => None,
        })
        .unwrap();
    let commands = respond(
        &mut model,
        request,
        ApiResponse::Sessions(Box::new(session_page())),
    );
    assert!(commands.contains(&Cmd::StartStream(StreamId::Transcript("sess-a".into()))));
}

#[test]
fn ut_585_busy_ctrl_x_confirms_posts_cancel_and_toasts() {
    let mut model = detail_model("active", true);
    update(
        &mut model,
        Msg::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)),
    );
    assert_eq!(
        model
            .session_detail()
            .unwrap()
            .confirm
            .as_ref()
            .unwrap()
            .prompt,
        "cancel the current turn? Enter/Esc"
    );
    insta::assert_snapshot!("session_cancel_confirm_100x30", render(&model, 100, 30));
    let request = update(&mut model, key(KeyCode::Enter))
        .into_iter()
        .find_map(|cmd| match cmd {
            Cmd::Post(request @ Request::CancelPrompt { .. }) => Some(request),
            _ => None,
        })
        .unwrap();
    respond(&mut model, request, ApiResponse::Empty);
    assert_eq!(model.toast.unwrap().text, "cancel requested");
}

#[test]
fn ut_586_idle_ctrl_x_only_toasts() {
    let mut model = detail_model("active", false);
    let commands = update(
        &mut model,
        Msg::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)),
    );
    assert!(!commands.iter().any(|cmd| matches!(cmd, Cmd::Post(_))));
    assert_eq!(model.toast.unwrap().text, "no turn in progress");
}

#[test]
fn ut_587_escape_cancels_cancel_confirmation() {
    let mut model = detail_model("active", true);
    update(
        &mut model,
        Msg::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)),
    );
    assert!(update(&mut model, key(KeyCode::Esc)).is_empty());
    assert!(model.session_detail().unwrap().confirm.is_none());
}

#[test]
fn ut_588_cancel_errors_show_daemon_message() {
    for error in ["HTTP 409: no active turn", "HTTP 404: session missing"] {
        let mut model = detail_model("active", true);
        let request = Request::CancelPrompt {
            id: batuta_tui::RequestId(9),
            workspace: "ws-test".into(),
            session: "sess-a".into(),
        };
        fail(&mut model, request, error);
        assert!(
            model
                .toast
                .unwrap()
                .text
                .contains(error.split(": ").last().unwrap())
        );
    }
}

#[test]
fn ut_642_reset_renders_resync_line_and_footer() {
    let mut model = detail_model("active", false);
    let snapshot = TranscriptSnapshot {
        epoch: 2,
        generation: 1,
        reset: true,
        reason: Some("cursor_gap".into()),
        ..Default::default()
    };
    update(
        &mut model,
        Msg::Stream {
            id: StreamId::Transcript("sess-a".into()),
            event: AnyStreamEvent::Transcript(TranscriptEvent::Snapshot(snapshot)),
        },
    );
    let screen = render(&model, 100, 30);
    assert!(screen.contains("resynchronized") && screen.contains("cursor_gap"));
}

#[test]
fn ut_644_daemon_restart_epoch_mismatch_resets_in_app() {
    let mut model = detail_model("active", false);
    let snapshot = TranscriptSnapshot {
        epoch: 9,
        generation: 1,
        reset: true,
        reason: Some("epoch_mismatch".into()),
        ..Default::default()
    };
    update(
        &mut model,
        Msg::Stream {
            id: StreamId::Transcript("sess-a".into()),
            event: AnyStreamEvent::Transcript(TranscriptEvent::Snapshot(snapshot)),
        },
    );
    assert!(
        matches!(model.session_detail().unwrap().view.footer, FooterState::Resynchronized(ref reason) if reason == "epoch_mismatch")
    );
}

#[test]
fn ut_650_conflicting_approve_refetches_without_repost() {
    let requests = [
        Request::Approve {
            id: batuta_tui::RequestId(7),
            workspace: "ws-test".into(),
            session: "sess-a".into(),
            request: ApproveRequest {
                request_id: "req".into(),
                turn_id: "turn".into(),
                decision: Decision::AllowOnce,
            },
        },
        Request::AnswerClarification {
            id: batuta_tui::RequestId(8),
            workspace: "ws-test".into(),
            session: "sess-a".into(),
            request_id: "clarify".into(),
            answer: ClarifyAnswer::Text("answer".into()),
        },
        Request::RunControl {
            id: batuta_tui::RequestId(9),
            workspace: "ws-test".into(),
            run: "run-a".into(),
            control: RunControl::Pause,
        },
    ];
    for request in requests {
        let mut model = detail_model("active", false);
        let commands = fail(&mut model, request, "HTTP 409: already changed (conflict)");
        assert!(commands.iter().any(|cmd| matches!(cmd, Cmd::Get(_))));
        assert!(!commands.iter().any(|cmd| matches!(cmd, Cmd::Post(_))));
        assert!(model.toast.unwrap().sticky);
    }
}

#[test]
fn ut_651_runtime_conflict_path_is_not_applicable_without_runtime_set() {
    let model = detail_model("active", false);
    assert!(
        !model
            .pending
            .values()
            .any(|pending| format!("{pending:?}").contains("RuntimeSet"))
    );
}
