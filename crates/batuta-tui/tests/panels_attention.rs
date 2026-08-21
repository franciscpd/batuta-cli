#[path = "support/fake_client.rs"]
mod fake_client;
#[path = "support/panels.rs"]
mod panels_support;
use batuta_tui::{
    Cmd, Msg, Request, StreamId, TimerId,
    app::{AttentionSource, Detail, Overlay, Panel},
    msg::ApiResponse,
    update,
};
use compozy_client::types::{
    Clarification, ClarifyResult, Decision, Overview, PermissionData, Session, TranscriptPage,
};
use crossterm::event::KeyCode;
use panels_support::*;

fn permission(id: &str, always: bool) -> PermissionData {
    serde_json::from_value(serde_json::json!({
        "request_id":id,"turn_id":"turn-1","title":"Bash: rm -rf build/","raw":{"tool_input":{"command":"rm -rf build/"},"options":if always { serde_json::json!([{"decision":"allow-once"},{"decision":"reject-once"},{"decision":"allow-always"},{"decision":"reject-always"}]) } else { serde_json::json!([{"decision":"allow-once"},{"decision":"reject-once"}]) }}
    })).unwrap()
}

fn clarification(id: &str, choices: Vec<&str>) -> Clarification {
    Clarification {
        request_id: id.into(),
        session_id: "sess-b".into(),
        agent_name: "batuta".into(),
        question: "Which environment?".into(),
        choices: choices.into_iter().map(str::to_owned).collect(),
        asked_at: Some("2025-08-18T23:55:00Z".to_owned().into()),
        deadline: Some("2025-08-19T00:10:00Z".to_owned().into()),
    }
}

fn overview(kind: &str, actions: &[&str], session: &str, run: &str) -> Overview {
    serde_json::from_value(serde_json::json!({"attention":{"total":1,"by_kind":{},"items":[{"kind":kind,"title":"task gate","detail":"needs operator","task_id":"task-7","run_id":run,"session_id":session,"occurred_at":"2025-08-18T23:58:00Z","actions":actions}]}})).unwrap()
}

fn populated() -> batuta_tui::Model {
    let mut model = model();
    respond(
        &mut model,
        sessions_request(300),
        ApiResponse::Sessions(Box::new(session_page())),
    );
    model
        .attention_permissions
        .insert("sess-a".into(), vec![permission("req-1", true)]);
    model.attention_clarifications.insert(
        "sess-b".into(),
        vec![clarification(
            "clar-1",
            vec!["staging", "production", "both"],
        )],
    );
    model.attention_overview = overview(
        "approval",
        &["approve", "reject", "open"],
        "",
        "looprun-parent1234",
    )
    .attention
    .items;
    model.attention_overview_total = 1;
    batuta_tui::app::panels::attention::rebuild(&mut model);
    model.focus = Panel::Attention;
    model
}

#[test]
fn ut_510_three_sources_sorted_oldest_first_snapshots() {
    let model = populated();
    assert!(matches!(
        model.attention[0].source,
        Some(AttentionSource::Permission { .. })
    ));
    assert!(matches!(
        model.attention[1].source,
        Some(AttentionSource::Clarification { .. })
    ));
    assert!(matches!(
        model.attention[2].source,
        Some(AttentionSource::Overview { .. })
    ));
    insta::assert_snapshot!("attention_three_sources_100x30", render(&model, 100, 30));
    insta::assert_snapshot!("attention_three_sources_120x40", render(&model, 120, 40));
}

#[test]
fn ut_511_all_refresh_triggers_fetch_sources() {
    let mut model = populated();
    for timer in [TimerId::CatalogDebounce, TimerId::AttentionPoll] {
        let commands = update(&mut model, Msg::Timer(timer));
        assert!(
            commands
                .iter()
                .any(|cmd| matches!(cmd, Cmd::Get(Request::Overview { .. })))
        );
        assert!(
            commands
                .iter()
                .any(|cmd| matches!(cmd, Cmd::Get(Request::Clarifications { .. })))
        );
    }
    let session = Session {
        id: "sess-a".into(),
        agent_name: "batuta".into(),
        state: "active".into(),
        ..Session::default()
    };
    model.detail = Detail::Session(Box::new(batuta_tui::app::SessionDetail::new(session)));
    model
        .active_streams
        .insert(StreamId::Transcript("sess-a".into()));
    let snapshot: compozy_client::types::TranscriptSnapshot = serde_json::from_value(serde_json::json!({"epoch":1,"generation":1,"entries":[{"message":{"id":"m","role":"assistant","parts":[{"type":"data-compozy-permission","data":{"request_id":"req-stream","turn_id":"turn-1","title":"Bash"}}]},"start_sequence":1,"sequence":1}],"max_sequence":1})).unwrap();
    let commands = update(
        &mut model,
        Msg::Stream {
            id: StreamId::Transcript("sess-a".into()),
            event: batuta_tui::AnyStreamEvent::Transcript(
                compozy_client::TranscriptEvent::Snapshot(snapshot),
            ),
        },
    );
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Get(Request::Overview { .. })))
    );
    assert_eq!(model.focus, Panel::Attention);
}

#[test]
fn ut_012_attention_navigation_changes_only_selection() {
    let mut model = populated();
    assert!(update(&mut model, key(KeyCode::Char('j'))).is_empty());
    assert_eq!(model.attention_selected, Some(1));
    assert_eq!(model.focus, Panel::Attention);
    assert!(matches!(model.detail, Detail::Empty));
}

#[test]
fn ut_512_count_and_more_title() {
    let mut model = populated();
    model.attention_overview_total = 11;
    assert!(render(&model, 100, 30).contains("Attention (3, +8 more)"));
}

#[test]
fn ut_513_enter_permission_and_run_open_context() {
    let mut model = populated();
    let commands = update(&mut model, key(KeyCode::Enter));
    assert_eq!(model.focus, Panel::Detail);
    assert!(commands.iter().any(
        |cmd| matches!(cmd, Cmd::Get(Request::Session { session, .. }) if session == "sess-a")
    ));
    model = populated();
    model.attention_selected = Some(2);
    let commands = update(&mut model, key(KeyCode::Enter));
    assert_eq!(model.focus, Panel::Detail);
    assert!(commands.iter().any(
        |cmd| matches!(cmd, Cmd::Get(Request::Run { run, .. }) if run == "looprun-parent1234")
    ));
}

#[test]
fn ut_514_visible_sessions_drive_clarification_and_transcript_reads() {
    let mut model = populated();
    model.sessions.filter = "spike".into();
    batuta_tui::app::panels::sessions::refilter(&mut model, None);
    assert_eq!(model.sessions.items.len(), 1);
    let commands = batuta_tui::app::panels::attention::refresh(&mut model);
    assert_eq!(
        commands
            .iter()
            .filter(|cmd| matches!(cmd, Cmd::Get(Request::Clarifications { .. })))
            .count(),
        2
    );
    assert_eq!(
        commands
            .iter()
            .filter(|cmd| matches!(cmd, Cmd::Get(Request::VisibleTranscript { .. })))
            .count(),
        2
    );
}

#[test]
fn ut_515_disallowed_action_is_noop_and_footer_lists_allowed() {
    let mut model = populated();
    model.attention_selected = Some(2);
    assert!(update(&mut model, key(KeyCode::Char('r'))).is_empty());
    let screen = render(&model, 100, 30);
    assert!(screen.contains("approve") && screen.contains("reject") && screen.contains("open"));
}

#[test]
fn ut_516_decided_permission_delta_removes_shared_item() {
    let mut model = populated();
    let page: TranscriptPage = serde_json::from_value(serde_json::json!({"entries":[{"message":{"id":"m","role":"assistant","parts":[{"type":"data-compozy-permission","data":{"request_id":"req-1","turn_id":"turn-1","decision":"allow-once"}}]},"start_sequence":1,"sequence":1}]})).unwrap();
    batuta_tui::app::panels::attention::apply_transcript(&mut model, "sess-a", &page);
    assert!(!model.attention.iter().any(|item| matches!(&item.source, Some(AttentionSource::Permission { request_id, .. }) if request_id == "req-1")));
}

#[test]
fn ut_517_permission_409_toast_and_refresh() {
    let mut model = populated();
    let request = permission_post(&mut model, KeyCode::Char('a'));
    let commands = fail(&mut model, request, "409 already decided");
    assert_eq!(model.toast.as_ref().unwrap().text, "already decided");
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Get(Request::Overview { .. })))
    );
}

#[test]
fn ut_518_overview_503_keeps_session_sources_and_dim_note() {
    let mut model = populated();
    fail(
        &mut model,
        Request::Overview {
            id: id(310),
            workspace: "ws-test".into(),
        },
        "503 unavailable",
    );
    assert!(model.attention_overview_unavailable);
    assert!(render(&model, 100, 30).contains("overview unavailable"));
}

#[test]
fn ut_519_empty_attention_snapshot() {
    let mut model = model();
    model.focus = Panel::Attention;
    insta::assert_snapshot!("attention_empty_100x30", render(&model, 100, 30));
}

fn permission_post(model: &mut batuta_tui::Model, code: KeyCode) -> Request {
    match update(model, key(code))
        .into_iter()
        .find_map(|cmd| match cmd {
            Cmd::Post(request) => Some(request),
            _ => None,
        }) {
        Some(request) => request,
        None => panic!("expected post"),
    }
}

#[test]
fn ut_520_allow_and_reject_once_post_and_toast() {
    for (code, decision, toast) in [
        (KeyCode::Char('a'), Decision::AllowOnce, "allowed"),
        (KeyCode::Char('x'), Decision::RejectOnce, "rejected"),
    ] {
        let mut model = populated();
        let request = permission_post(&mut model, code);
        assert!(
            matches!(&request, Request::Approve { request, .. } if request.decision == decision)
        );
        respond(&mut model, request, ApiResponse::Empty);
        assert_eq!(model.toast.as_ref().unwrap().text, toast);
    }
}

#[test]
fn ut_521_persistent_permission_requires_confirm_and_escape_cancels() {
    let mut model = populated();
    assert!(update(&mut model, key(KeyCode::Char('A'))).is_empty());
    assert!(model.attention_confirm.is_some());
    let request = permission_post(&mut model, KeyCode::Enter);
    assert!(
        matches!(request, Request::Approve { request, .. } if request.decision == Decision::AllowAlways)
    );
    let mut model = populated();
    update(&mut model, key(KeyCode::Char('X')));
    assert!(update(&mut model, key(KeyCode::Esc)).is_empty());
    assert!(model.attention_confirm.is_none());
}

#[test]
fn ut_522_escape_never_posts() {
    let mut model = populated();
    assert!(
        !update(&mut model, key(KeyCode::Esc))
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Post(_)))
    );
}

#[test]
fn ut_523_permission_card_fields_render() {
    let mut model = model();
    let session = Session {
        id: "sess-a".into(),
        agent_name: "batuta".into(),
        state: "active".into(),
        ..Session::default()
    };
    model.detail = Detail::Session(Box::new(batuta_tui::app::SessionDetail::new(session)));
    model.focus = Panel::Detail;
    let page: TranscriptPage = serde_json::from_value(serde_json::json!({"entries":[{"message":{"id":"m","role":"assistant","parts":[{"type":"data-compozy-permission","data":permission("req-card",true)}]},"start_sequence":1,"sequence":1}]})).unwrap();
    batuta_tui::app::page_into_detail(model.session_detail_mut().unwrap(), page);
    model.session_detail_mut().unwrap().view.cache_dirty = true;
    update(&mut model, Msg::Tick);
    let screen = render(&model, 100, 30);
    assert!(
        screen.contains("request req-card")
            && screen.contains("turn turn-1")
            && screen.contains("rm -rf build/")
    );
}

#[test]
fn ut_524_inline_permission_card_uses_same_verb() {
    let mut model = model();
    let session = Session {
        id: "sess-a".into(),
        agent_name: "batuta".into(),
        state: "active".into(),
        ..Session::default()
    };
    model.detail = Detail::Session(Box::new(batuta_tui::app::SessionDetail::new(session)));
    model.focus = Panel::Detail;
    let page: TranscriptPage = serde_json::from_value(serde_json::json!({"entries":[{"message":{"id":"m","role":"assistant","parts":[{"type":"data-compozy-permission","data":permission("req-inline",true)}]},"start_sequence":1,"sequence":1}]})).unwrap();
    batuta_tui::app::page_into_detail(model.session_detail_mut().unwrap(), page);
    assert!(
        matches!(permission_post(&mut model, KeyCode::Char('a')), Request::Approve { request, .. } if request.request_id == "req-inline")
    );
}

#[test]
fn ut_525_permission_409_refreshes() {
    let mut model = populated();
    let request = permission_post(&mut model, KeyCode::Char('a'));
    let commands = fail(&mut model, request, "409 already decided");
    assert_eq!(model.toast.as_ref().unwrap().text, "already decided");
    assert!(commands.iter().any(
        |cmd| matches!(cmd, Cmd::Get(Request::Clarifications { session, .. }) if session == "sess-a")
    ));
}

#[test]
fn ut_526_permission_404_removes_item() {
    let mut model = populated();
    let request = permission_post(&mut model, KeyCode::Char('a'));
    fail(&mut model, request, "404 session not found");
    assert!(
        !model
            .attention
            .iter()
            .any(|item| matches!(item.source, Some(AttentionSource::Permission { .. })))
    );
}

#[test]
fn ut_527_two_permissions_are_independent() {
    let mut model = populated();
    model
        .attention_permissions
        .get_mut("sess-a")
        .unwrap()
        .push(permission("req-2", true));
    batuta_tui::app::panels::attention::rebuild(&mut model);
    assert_eq!(
        model
            .attention
            .iter()
            .filter(|item| matches!(item.source, Some(AttentionSource::Permission { .. })))
            .count(),
        2
    );
}

#[test]
fn ut_528_missing_always_option_disables_uppercase() {
    let mut model = populated();
    model
        .attention_permissions
        .insert("sess-a".into(), vec![permission("req-1", false)]);
    batuta_tui::app::panels::attention::rebuild(&mut model);
    assert!(update(&mut model, key(KeyCode::Char('A'))).is_empty());
    assert!(model.attention_confirm.is_none());
}

fn choose_model(choices: Vec<&str>) -> batuta_tui::Model {
    let mut model = populated();
    model.attention_selected = Some(1);
    model
        .attention_clarifications
        .insert("sess-b".into(), vec![clarification("clar-1", choices)]);
    batuta_tui::app::panels::attention::rebuild(&mut model);
    model.attention_selected = model
        .attention
        .iter()
        .position(|item| matches!(item.source, Some(AttentionSource::Clarification { .. })));
    update(&mut model, key(KeyCode::Enter));
    model
}

#[test]
fn ut_530_choice_chooser_snapshot_and_zero_based_answer() {
    let mut model = choose_model(vec!["staging", "production", "both"]);
    insta::assert_snapshot!("clarification_choices_100x30", render(&model, 100, 30));
    update(&mut model, key(KeyCode::Char('2')));
    let request = permission_post(&mut model, KeyCode::Enter);
    assert!(matches!(
        request,
        Request::AnswerClarification {
            answer: compozy_client::types::ClarifyAnswer::Choice(1),
            ..
        }
    ));
}

#[test]
fn ut_531_free_text_answer_posts() {
    let mut model = choose_model(vec![]);
    for ch in "staging".chars() {
        update(&mut model, key(KeyCode::Char(ch)));
    }
    let request = permission_post(&mut model, KeyCode::Enter);
    assert!(
        matches!(request, Request::AnswerClarification { answer:compozy_client::types::ClarifyAnswer::Text(ref text), .. } if text == "staging")
    );
}

#[test]
fn ut_532_answer_success_closes_and_refreshes() {
    let mut model = choose_model(vec!["a"]);
    let request = permission_post(&mut model, KeyCode::Enter);
    let commands = respond(
        &mut model,
        request,
        ApiResponse::ClarificationAnswered(ClarifyResult::default()),
    );
    assert!(model.overlay.is_none());
    assert_eq!(model.toast.as_ref().unwrap().text, "answered");
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Get(Request::Overview { .. })))
    );
}

#[test]
fn ut_533_expired_answer_404() {
    let mut model = choose_model(vec!["a"]);
    let request = permission_post(&mut model, KeyCode::Enter);
    fail(&mut model, request, "404 expired");
    assert_eq!(
        model.toast.unwrap().text,
        "clarification expired or already answered"
    );
}

#[test]
fn ut_534_answer_409_503_and_draining_messages() {
    for (error, expected) in [
        ("409 daemon says no", "409 daemon says no"),
        ("503 unavailable", "clarification service unavailable"),
        (
            "503 daemon is draining",
            "daemon is draining — writes refused",
        ),
    ] {
        let mut model = choose_model(vec!["a"]);
        let request = permission_post(&mut model, KeyCode::Enter);
        fail(&mut model, request, error);
        assert_eq!(model.toast.unwrap().text, expected);
    }
}

#[test]
fn ut_535_empty_text_hints_without_post() {
    let mut model = choose_model(vec![]);
    assert!(update(&mut model, key(KeyCode::Enter)).is_empty());
    assert!(
        matches!(model.overlay, Some(Overlay::Clarify { hint:Some(ref hint), .. }) if hint == "type an answer")
    );
}

#[test]
fn ut_536_six_choices_support_digits_and_jk() {
    let mut model = choose_model(vec!["1", "2", "3", "4", "5", "6"]);
    update(&mut model, key(KeyCode::Char('6')));
    assert!(matches!(
        model.overlay,
        Some(Overlay::Clarify { selected: 5, .. })
    ));
    update(&mut model, key(KeyCode::Char('k')));
    assert!(matches!(
        model.overlay,
        Some(Overlay::Clarify { selected: 4, .. })
    ));
}

fn overview_model(kind: &str, actions: &[&str], session: &str, run: &str) -> batuta_tui::Model {
    let mut model = model();
    respond(
        &mut model,
        sessions_request(330),
        ApiResponse::Sessions(Box::new(session_page())),
    );
    model.attention_overview = overview(kind, actions, session, run).attention.items;
    model.attention_overview_total = 1;
    batuta_tui::app::panels::attention::rebuild(&mut model);
    model.focus = Panel::Attention;
    model
}

#[test]
fn ut_540_overview_approve_and_reject() {
    for (code, verb) in [
        (KeyCode::Char('a'), compozy_client::TaskVerb::Approve),
        (KeyCode::Char('x'), compozy_client::TaskVerb::Reject),
    ] {
        let mut model = overview_model("approval", &["approve", "reject"], "", "");
        assert!(
            matches!(permission_post(&mut model, code), Request::TaskVerb { verb:v, .. } if v == verb)
        );
    }
}

#[test]
fn ut_541_failure_retry_toast() {
    let mut model = overview_model("failure", &["retry"], "", "run-1");
    let request = permission_post(&mut model, KeyCode::Char('r'));
    assert!(matches!(
        request,
        Request::TaskVerb {
            verb: compozy_client::TaskVerb::Retry,
            ..
        }
    ));
    respond(&mut model, request, ApiResponse::Empty);
    assert_eq!(model.toast.unwrap().text, "retry requested");
}

#[test]
fn ut_542_open_session_run_or_web() {
    let mut session = overview_model("needs_input", &["open"], "sess-a", "");
    assert!(
        update(&mut session, key(KeyCode::Enter))
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Get(Request::Session { .. })))
    );
    let mut run = overview_model("failure", &["open"], "", "run-1");
    assert!(
        update(&mut run, key(KeyCode::Char('o')))
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Get(Request::Run { .. })))
    );
    assert_eq!(run.focus, Panel::Detail);
    let mut web = overview_model("needs_input", &["open"], "", "");
    update(&mut web, key(KeyCode::Enter));
    assert!(web.toast.unwrap().text.contains("open in web"));
}

#[test]
fn ut_543_keys_outside_actions_noop() {
    let mut model = overview_model("approval", &["open"], "", "");
    assert!(update(&mut model, key(KeyCode::Char('a'))).is_empty());
}

#[test]
fn ut_544_any_overview_verb_refetches() {
    let mut model = overview_model("approval", &["approve"], "", "");
    let request = permission_post(&mut model, KeyCode::Char('a'));
    let commands = respond(&mut model, request, ApiResponse::Empty);
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Get(Request::Overview { .. })))
    );
}

#[test]
fn ut_545_off_page_target_fetches_directly_without_filter_change() {
    let mut model = overview_model("needs_input", &["open"], "sess-off-page", "");
    model.sessions.filter = "spike".into();
    let commands = update(&mut model, key(KeyCode::Enter));
    assert!(commands.iter().any(|cmd| matches!(cmd, Cmd::Get(Request::Session { session, .. }) if session == "sess-off-page")));
    assert_eq!(model.sessions.filter, "spike");
}

#[test]
fn ut_546_approve_409_toasts_and_refreshes() {
    let mut model = overview_model("approval", &["approve"], "", "");
    let request = permission_post(&mut model, KeyCode::Char('a'));
    let commands = fail(&mut model, request, "409 already approved");
    assert!(model.toast.is_some());
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Get(Request::Overview { .. })))
    );
}

#[tokio::test]
async fn fake_runtime_client_records_panel_request() {
    use batuta_tui::runtime::RuntimeClient;
    use fake_client::FakeRuntimeClient;
    let fake = FakeRuntimeClient::default();
    fake.push_response(Ok(ApiResponse::Empty));
    fake.script_stream(StreamId::Catalog, Vec::new());
    fake.set_pending(false);
    let request = Request::Overview {
        id: id(999),
        workspace: "ws-test".into(),
    };
    fake.get(request.clone()).await.unwrap();
    assert_eq!(fake.requests(), vec![request]);
}
