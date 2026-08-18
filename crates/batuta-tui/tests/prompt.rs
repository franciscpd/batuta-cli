#[path = "support/panels.rs"]
mod panels_support;

use batuta_tui::{
    Cmd, Request,
    app::{Detail, Panel, SessionDetail},
    msg::ApiResponse,
    update,
};
use compozy_client::types::{Activity, PromptMode, PromptOutcome, PromptResult, Session};
use crossterm::event::KeyCode;
use panels_support::{fail, key, model, respond};

fn prompt_model(busy: bool) -> batuta_tui::Model {
    let mut model = model();
    let activity = busy.then(|| Activity {
        turn_id: Some("turn-1".into()),
        ..Default::default()
    });
    model.detail = Detail::Session(Box::new(SessionDetail::new(Session {
        id: "sess-a".into(),
        agent_name: "batuta".into(),
        state: "active".into(),
        badge: busy.then(|| "running".into()),
        activity,
        ..Default::default()
    })));
    model.focus = Panel::Detail;
    model.session_detail_mut().unwrap().composer.focused = true;
    model
        .session_detail_mut()
        .unwrap()
        .composer
        .set_text("hello");
    model
}

fn prompt_request(commands: &[Cmd]) -> Request {
    commands
        .iter()
        .find_map(|command| match command {
            Cmd::Post(request @ Request::Prompt { .. }) => Some(request.clone()),
            _ => None,
        })
        .expect("prompt post")
}

#[test]
fn ut_561_idle_send_has_queue_mode_ids_and_clears_on_success() {
    let mut model = prompt_model(false);
    let request = prompt_request(&update(&mut model, key(KeyCode::Enter)));
    match &request {
        Request::Prompt {
            prompt,
            consume_body,
            ..
        } => {
            assert_eq!(prompt.mode, PromptMode::Queue);
            assert!(prompt.expected_turn_id.is_none());
            assert!(prompt.message_id.starts_with("msg_") && prompt.message_id.len() == 20);
            assert!(
                prompt.idempotency_key.starts_with("idem_") && prompt.idempotency_key.len() == 21
            );
            assert!(!consume_body);
        }
        _ => unreachable!(),
    }
    respond(
        &mut model,
        request,
        ApiResponse::Prompt(PromptOutcome::Sent),
    );
    assert!(model.session_detail().unwrap().composer.is_empty());
}

#[test]
fn ut_562_busy_send_uses_mode_chooser_and_expected_turn() {
    let mut model = prompt_model(true);
    assert!(update(&mut model, key(KeyCode::Enter)).is_empty());
    assert!(model.session_detail().unwrap().chooser.is_some());
    insta::assert_snapshot!(
        "session_mode_chooser_100x30",
        panels_support::render(&model, 100, 30)
    );
    let queue = prompt_request(&update(&mut model, key(KeyCode::Enter)));
    assert!(
        matches!(queue, Request::Prompt { ref prompt, .. } if prompt.mode == PromptMode::Queue && prompt.expected_turn_id.as_deref() == Some("turn-1"))
    );

    let mut model = prompt_model(true);
    update(&mut model, key(KeyCode::Enter));
    update(&mut model, key(KeyCode::Char('j')));
    let steer = prompt_request(&update(&mut model, key(KeyCode::Enter)));
    assert!(
        matches!(steer, Request::Prompt { ref prompt, .. } if prompt.mode == PromptMode::Steer)
    );

    let mut model = prompt_model(true);
    update(&mut model, key(KeyCode::Enter));
    update(&mut model, key(KeyCode::Esc));
    assert!(model.session_detail().unwrap().chooser.is_none());
}

#[test]
fn ut_563_new_send_ids_change_and_failed_same_draft_retry_reuses_ids() {
    let mut model = prompt_model(false);
    let first = prompt_request(&update(&mut model, key(KeyCode::Enter)));
    fail(&mut model, first.clone(), "transport: reset");
    let retry = prompt_request(&update(&mut model, key(KeyCode::Enter)));
    let ids = |request: &Request| match request {
        Request::Prompt { prompt, .. } => {
            (prompt.message_id.clone(), prompt.idempotency_key.clone())
        }
        _ => unreachable!(),
    };
    assert_eq!(ids(&first), ids(&retry));
    respond(&mut model, retry, ApiResponse::Prompt(PromptOutcome::Sent));
    model
        .session_detail_mut()
        .unwrap()
        .composer
        .set_text("next");
    let next = prompt_request(&update(&mut model, key(KeyCode::Enter)));
    assert_ne!(ids(&first), ids(&next));
}

#[test]
fn ut_564_prompt_requests_never_consume_the_response_body() {
    let mut model = prompt_model(false);
    let request = prompt_request(&update(&mut model, key(KeyCode::Enter)));
    assert!(matches!(
        request,
        Request::Prompt {
            consume_body: false,
            ..
        }
    ));
}

#[test]
fn ut_565_failed_send_keeps_the_draft() {
    let mut model = prompt_model(false);
    let request = prompt_request(&update(&mut model, key(KeyCode::Enter)));
    fail(&mut model, request, "HTTP 400: bad prompt");
    assert_eq!(model.session_detail().unwrap().composer.text(), "hello");
}

fn finish(outcome: PromptOutcome) -> batuta_tui::Model {
    let mut model = prompt_model(false);
    let request = prompt_request(&update(&mut model, key(KeyCode::Enter)));
    respond(&mut model, request, ApiResponse::Prompt(outcome));
    model
}

#[test]
fn ut_570_sent_toast_expires() {
    let mut model = prompt_model(false);
    let request = prompt_request(&update(&mut model, key(KeyCode::Enter)));
    let commands = respond(
        &mut model,
        request,
        ApiResponse::Prompt(PromptOutcome::Sent),
    );
    assert_eq!(model.toast.as_ref().unwrap().text, "sent");
    assert!(commands.contains(&Cmd::After(
        std::time::Duration::from_secs(3),
        batuta_tui::TimerId::ToastExpiry,
    )));
    assert!(
        update(
            &mut model,
            batuta_tui::Msg::Timer(batuta_tui::TimerId::ToastExpiry)
        )
        .is_empty()
    );
    assert!(model.toast.is_none());
}

#[test]
fn ut_571_accepted_outcomes_map_delivery_to_toasts() {
    for (delivery, mode, position, expected) in [
        (
            "after_turn",
            PromptMode::Queue,
            Some(2),
            "queued · position 2",
        ),
        (
            "interrupt_then_prompt",
            PromptMode::Interrupt,
            None,
            "interrupting",
        ),
        ("steer", PromptMode::Steer, None, "steering"),
    ] {
        let model = finish(PromptOutcome::Accepted(PromptResult {
            delivery: delivery.into(),
            mode,
            queue_position: position,
            ..Default::default()
        }));
        assert_eq!(model.toast.unwrap().text, expected);
    }
}

fn failed_prompt(error: &str) -> batuta_tui::Model {
    let mut model = prompt_model(false);
    let request = prompt_request(&update(&mut model, key(KeyCode::Enter)));
    fail(&mut model, request, error);
    model
}

#[test]
fn ut_572_prompt_409_is_indeterminate_and_sticky() {
    let model = failed_prompt("HTTP 409: uncertain");
    let toast = model.toast.unwrap();
    assert!(toast.sticky);
    assert_eq!(
        toast.text,
        "check the session — indeterminate dispatch (uncertain)"
    );
}

#[test]
fn ut_573_prompt_413_shows_queue_capacity() {
    assert_eq!(
        failed_prompt("HTTP 413: queue full; queue_cap=10")
            .toast
            .unwrap()
            .text,
        "input queue full (cap 10)"
    );
}

#[test]
fn ut_574_prompt_503_has_draining_copy() {
    assert_eq!(
        failed_prompt("HTTP 503: daemon draining")
            .toast
            .unwrap()
            .text,
        "daemon is draining — writes refused"
    );
}

#[test]
fn ut_575_active_turn_mismatch_has_specific_copy() {
    assert_eq!(
        failed_prompt("HTTP 409: active turn mismatch")
            .toast
            .unwrap()
            .text,
        "turn changed — check the session and send again"
    );
}

#[test]
fn ut_576_sticky_toast_dismisses_and_escape_is_consumed() {
    let mut model = failed_prompt("HTTP 409: uncertain");
    update(&mut model, key(KeyCode::Esc));
    assert!(model.toast.is_none());
    assert!(model.session_detail().unwrap().composer.focused);

    let mut model = failed_prompt("HTTP 409: uncertain");
    update(&mut model, key(KeyCode::Char('z')));
    assert!(model.toast.is_none());
    assert_eq!(model.session_detail().unwrap().composer.text(), "helloz");
}

#[test]
fn ut_577_transport_error_has_safe_retry_copy() {
    assert_eq!(
        failed_prompt("transport: connection reset")
            .toast
            .unwrap()
            .text,
        "request failed — check the session before retrying"
    );
}

#[test]
fn ut_578_pending_send_ignores_second_enter() {
    let mut model = prompt_model(false);
    assert_eq!(
        update(&mut model, key(KeyCode::Enter))
            .iter()
            .filter(|c| matches!(c, Cmd::Post(_)))
            .count(),
        1
    );
    assert!(update(&mut model, key(KeyCode::Enter)).is_empty());
}

#[test]
fn other_prompt_4xx_preserves_daemon_message_and_code() {
    assert_eq!(
        failed_prompt("HTTP 400: invalid prompt (invalid_prompt)")
            .toast
            .unwrap()
            .text,
        "invalid prompt (invalid_prompt)"
    );
}
