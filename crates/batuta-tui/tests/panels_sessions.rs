#[path = "support/panels.rs"]
mod panels_support;
use batuta_tui::{
    Cmd, Msg, Request, StreamId, TimerId,
    app::{Detail, Panel},
    msg::{AnyStreamEvent, ApiResponse},
    update,
};
use compozy_client::{
    Error, StreamEvent,
    types::{CatalogEvent, PromptOutcome, Session, SessionPage},
};
use crossterm::event::KeyCode;
use panels_support::*;
use std::time::Duration;

fn load(model: &mut batuta_tui::Model) -> Vec<Cmd> {
    respond(
        model,
        sessions_request(100),
        ApiResponse::Sessions(Box::new(session_page())),
    )
}

#[test]
fn ut_460_sessions_rows_snapshot() {
    let mut model = model();
    load(&mut model);
    insta::assert_snapshot!("sessions_populated_100x30", render(&model, 100, 30));
}

#[test]
fn ut_461_star_toggles_agent_query_and_title() {
    let mut model = model();
    load(&mut model);
    let commands = update(&mut model, key(KeyCode::Char('*')));
    assert!(matches!(
        &commands[0],
        Cmd::Get(Request::Sessions { agent: None, .. })
    ));
    assert!(render(&model, 100, 30).contains("Sessions · all agents"));
    let commands = update(&mut model, key(KeyCode::Char('*')));
    assert!(
        matches!(&commands[0], Cmd::Get(Request::Sessions { agent: Some(agent), .. }) if agent == "batuta")
    );
}

#[test]
fn ut_462_filter_enter_keeps_and_escape_clears() {
    let mut model = model();
    load(&mut model);
    update(&mut model, key(KeyCode::Char('/')));
    for ch in "spi".chars() {
        update(&mut model, key(KeyCode::Char(ch)));
    }
    assert_eq!(model.sessions.items.len(), 1);
    update(&mut model, key(KeyCode::Enter));
    assert_eq!(model.sessions.filter, "spi");
    insta::assert_snapshot!("sessions_filtered_100x30", render(&model, 100, 30));
    update(&mut model, key(KeyCode::Char('/')));
    update(&mut model, key(KeyCode::Esc));
    assert!(model.sessions.filter.is_empty());
    assert_eq!(model.sessions.items.len(), 2);
}

#[test]
fn ut_463_enter_opens_session_with_all_reads_and_stream() {
    let mut model = model();
    load(&mut model);
    let commands = update(&mut model, key(KeyCode::Enter));
    assert!(matches!(model.detail, Detail::Session(ref detail) if detail.session.id == "sess-a"));
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Get(Request::Session { .. })))
    );
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Get(Request::TranscriptPage { .. })))
    );
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Get(Request::Clarifications { .. })))
    );
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::StartStream(StreamId::Transcript(id)) if id == "sess-a"))
    );
}

#[test]
fn ut_464_has_more_note() {
    let mut model = model();
    load(&mut model);
    assert!(render(&model, 100, 30).contains("more (limit 50)"));
}

#[test]
fn ut_465_empty_snapshot() {
    let model = model();
    insta::assert_snapshot!("sessions_empty_100x30", render(&model, 100, 30));
}

#[test]
fn ut_466_archived_row_disappears_and_selection_clamps() {
    let mut model = model();
    load(&mut model);
    model.sessions.selected = Some(1);
    let mut page = session_page();
    page.sessions.remove(1);
    respond(
        &mut model,
        sessions_request(101),
        ApiResponse::Sessions(Box::new(page)),
    );
    assert_eq!(model.sessions.items.len(), 1);
    assert_eq!(model.sessions.selected, Some(0));
}

#[test]
fn ut_467_no_match_state() {
    let mut model = model();
    load(&mut model);
    model.sessions.filter = "missing".into();
    batuta_tui::app::panels::sessions::refilter(&mut model, None);
    assert!(render(&model, 100, 30).contains("no match"));
}

#[test]
fn ut_468_large_page_renders_only_area_height() {
    let mut model = model();
    let page = SessionPage {
        sessions: (0..100)
            .map(|index| Session {
                id: format!("sess-{index}"),
                agent_name: "batuta".into(),
                state: "active".into(),
                ..Session::default()
            })
            .collect(),
        ..SessionPage::default()
    };
    respond(
        &mut model,
        sessions_request(102),
        ApiResponse::Sessions(Box::new(page)),
    );
    assert!(
        render(&model, 100, 30)
            .lines()
            .filter(|line| line.contains("sess-"))
            .count()
            < 30
    );
}

fn catalog(model: &mut batuta_tui::Model, workspace: &str) -> Vec<Cmd> {
    model.active_streams.insert(StreamId::Catalog);
    update(
        model,
        Msg::Stream {
            id: StreamId::Catalog,
            event: AnyStreamEvent::Catalog(StreamEvent::Event(CatalogEvent {
                kind: "upserted".into(),
                workspace_id: workspace.into(),
                session_id: "sess-new".into(),
            })),
        },
    )
}

#[test]
fn ut_470_catalog_wake_debounces_then_refetches() {
    let mut model = model();
    assert_eq!(
        catalog(&mut model, "ws-test"),
        vec![Cmd::After(
            Duration::from_millis(300),
            TimerId::CatalogDebounce
        )]
    );
    let commands = update(&mut model, Msg::Timer(TimerId::CatalogDebounce));
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Get(Request::Sessions { .. })))
    );
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Get(Request::Overview { .. })))
    );
}

#[test]
fn ut_471_twenty_catalog_events_arm_once() {
    let mut model = model();
    let count = (0..20)
        .flat_map(|_| catalog(&mut model, "ws-test"))
        .filter(|cmd| matches!(cmd, Cmd::After(_, TimerId::CatalogDebounce)))
        .count();
    assert_eq!(count, 1);
}

#[test]
fn ut_472_other_workspace_event_is_ignored() {
    let mut model = model();
    assert!(catalog(&mut model, "ws-other").is_empty());
}

#[test]
fn ut_473_catalog_503_falls_back_to_polling() {
    let mut model = model();
    model.active_streams.insert(StreamId::Catalog);
    let commands = update(
        &mut model,
        Msg::Stream {
            id: StreamId::Catalog,
            event: AnyStreamEvent::Catalog(StreamEvent::Fatal(Error::Daemon {
                status: 503,
                message: "subscriber unavailable".into(),
                code: None,
                details: None,
            })),
        },
    );
    assert_eq!(
        commands,
        vec![Cmd::After(Duration::from_secs(10), TimerId::CatalogPoll)]
    );
    assert!(render(&model, 100, 30).contains("catalog: polling"));
}

#[test]
fn ut_474_failed_refetch_keeps_rows_and_toasts() {
    let mut model = model();
    load(&mut model);
    fail(&mut model, sessions_request(103), "catalog read failed");
    assert_eq!(model.sessions.items.len(), 2);
    assert_eq!(
        model.toast.as_ref().map(|toast| toast.text.as_str()),
        Some("catalog read failed")
    );
}

fn created() -> Session {
    Session {
        id: "sess-created".into(),
        agent_name: "batuta".into(),
        state: "active".into(),
        ..Session::default()
    }
}

#[test]
fn ut_480_new_session_refetches_selects_and_focuses_composer() {
    let mut model = model();
    let commands = update(&mut model, key(KeyCode::Char('n')));
    let request = match &commands[0] {
        Cmd::Post(request @ Request::CreateSession { .. }) => request.clone(),
        _ => panic!(),
    };
    respond(&mut model, request, ApiResponse::SessionCreated(created()));
    assert!(
        matches!(model.detail, Detail::Session(ref detail) if detail.session.id == "sess-created" && detail.composer.focused)
    );
    assert_eq!(model.focus, Panel::Detail);
    assert_eq!(
        model.sessions.selected().map(|row| row.id.as_str()),
        Some("sess-created")
    );
}

#[test]
fn ut_481_first_prompt_has_runtime_later_prompt_does_not() {
    let mut model = model();
    let create = match update(&mut model, key(KeyCode::Char('n'))).remove(0) {
        Cmd::Post(request) => request,
        _ => panic!(),
    };
    respond(&mut model, create, ApiResponse::SessionCreated(created()));
    model.session_detail_mut().unwrap().composer.text = "hello".into();
    let prompt = match update(&mut model, key(KeyCode::Enter)).remove(0) {
        Cmd::Post(request) => request,
        _ => panic!(),
    };
    assert!(
        matches!(&prompt, Request::Prompt { prompt, .. } if prompt.runtime.as_ref().is_some_and(|runtime| runtime.provider == "claude" && runtime.model.is_none()))
    );
    let id = prompt.id();
    update(
        &mut model,
        Msg::Api {
            request: id,
            result: Ok(ApiResponse::Prompt(PromptOutcome::Sent)),
        },
    );
    model.session_detail_mut().unwrap().composer.text = "again".into();
    let prompt = match update(&mut model, key(KeyCode::Enter)).remove(0) {
        Cmd::Post(request) => request,
        _ => panic!(),
    };
    assert!(matches!(prompt, Request::Prompt { prompt, .. } if prompt.runtime.is_none()));
}

#[test]
fn ut_482_create_errors_have_specific_toasts() {
    for (error, expected) in [
        (
            "400 unknown agent",
            "agent batuta not found in this workspace",
        ),
        ("503 unavailable", "daemon is draining"),
        ("409 conflict message", "409 conflict message"),
    ] {
        let mut model = model();
        fail(
            &mut model,
            Request::CreateSession {
                id: id(7),
                workspace: "ws-test".into(),
                agent: "batuta".into(),
            },
            error,
        );
        assert_eq!(model.toast.unwrap().text, expected);
    }
}

#[test]
fn ut_483_second_n_is_disabled_while_pending() {
    let mut model = model();
    assert_eq!(update(&mut model, key(KeyCode::Char('n'))).len(), 1);
    assert!(update(&mut model, key(KeyCode::Char('n'))).is_empty());
}

#[test]
fn ut_484_n_is_enabled_after_completion() {
    let mut model = model();
    let first = match update(&mut model, key(KeyCode::Char('n'))).remove(0) {
        Cmd::Post(request) => request,
        _ => panic!(),
    };
    respond(&mut model, first, ApiResponse::SessionCreated(created()));
    model.session_detail_mut().unwrap().composer.focused = false;
    model.focus = Panel::Sessions;
    assert!(matches!(
        update(&mut model, key(KeyCode::Char('n'))).first(),
        Some(Cmd::Post(Request::CreateSession { .. }))
    ));
}

#[test]
fn ut_485_empty_provider_blocks_first_prompt() {
    let mut model = model();
    model.settings.preset.provider.clear();
    model.detail = Detail::Session(Box::new(batuta_tui::app::SessionDetail::new(created())));
    model.app_created_sessions.insert("sess-created".into());
    model.session_detail_mut().unwrap().composer.text = "hello".into();
    model.session_detail_mut().unwrap().composer.focused = true;
    assert!(update(&mut model, key(KeyCode::Enter)).is_empty());
    assert_eq!(model.toast.unwrap().text, "set preset.provider in config");
}

#[test]
fn navigation_and_refresh_keys_never_post() {
    let mut model = model();
    load(&mut model);
    for code in [
        KeyCode::Char('j'),
        KeyCode::Char('k'),
        KeyCode::Enter,
        KeyCode::Char('/'),
        KeyCode::Char('*'),
        KeyCode::Char('r'),
    ] {
        model.focus = Panel::Sessions;
        assert!(
            !update(&mut model, key(code))
                .iter()
                .any(|cmd| matches!(cmd, Cmd::Post(_)))
        );
    }
}
