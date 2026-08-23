#[path = "support/panels.rs"]
mod panels_support;
use batuta_tui::{Cmd, Request, RequestId, msg::ApiResponse};
use panels_support::*;

fn overview_response() -> compozy_client::types::Overview {
    serde_json::from_value(serde_json::json!({
        "attention": {
            "total": 1,
            "by_kind": {},
            "items": [{
                "kind": "approval",
                "title": "task gate",
                "detail": "needs operator",
                "task_id": "task-7",
                "run_id": "looprun-parent1234",
                "session_id": "",
                "occurred_at": "2025-08-18T23:58:00Z",
                "actions": ["approve", "reject", "open"]
            }]
        }
    }))
    .unwrap()
}

fn overview_request(id: u64) -> Request {
    Request::Overview {
        id: RequestId(id),
        workspace: "ws-test".into(),
    }
}

fn deliver_attention_item(model: &mut batuta_tui::Model) -> Vec<Cmd> {
    respond(
        model,
        overview_request(900),
        ApiResponse::Overview(Box::new(overview_response())),
    )
}

#[test]
fn notify_fires_only_when_unfocused_and_enabled() {
    let mut unfocused = model();
    unfocused.terminal_focused = false;
    let cmds = deliver_attention_item(&mut unfocused);
    assert!(cmds.iter().any(|c| matches!(c, Cmd::Notify { .. })));

    let mut focused = model();
    focused.terminal_focused = true;
    let cmds = deliver_attention_item(&mut focused);
    assert!(!cmds.iter().any(|c| matches!(c, Cmd::Notify { .. })));

    let mut disabled = model();
    disabled.terminal_focused = false;
    disabled.settings.ui.notify = false;
    let cmds = deliver_attention_item(&mut disabled);
    assert!(!cmds.iter().any(|c| matches!(c, Cmd::Notify { .. })));
}
