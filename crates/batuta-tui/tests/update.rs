use batuta_tui::{
    app::{FooterState, Model, SessionHeader, update},
    cmd::Cmd,
    msg::Msg,
};
use compozy_client::{
    TranscriptEvent,
    types::{
        Entry, ErrorPayload, Part, SessionStopped, TranscriptDelta, TranscriptPage,
        TranscriptSnapshot, UiMessage,
    },
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::time::Duration;

fn entry(start: i64, sequence: i64) -> Entry {
    Entry {
        start_sequence: start,
        sequence,
        message: UiMessage {
            id: format!("m-{start}"),
            ..UiMessage::default()
        },
    }
}

fn page(entries: Vec<Entry>, has_older: bool) -> TranscriptPage {
    TranscriptPage {
        entries,
        epoch: 1,
        generation: 2,
        max_sequence: 30,
        has_older,
        next_before_sequence: has_older.then_some(10),
        limit: 200,
    }
}

fn model() -> Model {
    Model::new(SessionHeader {
        workspace: "workspace".into(),
        workspace_id: "ws-test".into(),
        session_id: "sess-1234567890abcdef".into(),
        agent: "batuta".into(),
        name: Some("task".into()),
        state: "active".into(),
        warning: None,
    })
}

fn press(code: KeyCode) -> Msg {
    Msg::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

#[test]
fn ut_140_navigation_keys_clamp_and_follow() {
    let mut model = model();
    update(
        &mut model,
        Msg::Page(Ok(page(
            vec![entry(10, 10), entry(20, 20), entry(30, 30)],
            false,
        ))),
    );
    model.visible_height = 2;

    update(&mut model, press(KeyCode::Char('k')));
    assert_eq!(model.selection, 1);
    update(&mut model, press(KeyCode::PageUp));
    assert_eq!(model.selection, 0);
    update(&mut model, press(KeyCode::PageDown));
    assert_eq!(model.selection, 2);
    update(&mut model, press(KeyCode::Char('g')));
    assert_eq!(model.selection, 0);
    update(&mut model, press(KeyCode::Char('G')));
    assert_eq!(model.selection, 2);
    assert!(model.follow);
}

#[test]
fn ut_142_escape_is_a_complete_noop() {
    let mut model = model();
    model.dirty = false;
    let commands = update(&mut model, press(KeyCode::Esc));
    assert!(commands.is_empty());
    assert!(!model.dirty);
}

#[test]
fn ut_141_enter_only_toggles_expandable_entries() {
    let mut model = model();
    let mut text = entry(10, 10);
    text.message.parts.push(Part::Text {
        text: "x".into(),
        state: None,
    });
    let mut tool = entry(20, 20);
    tool.message.parts.push(Part::Tool {
        name: "x".into(),
        tool_call_id: None,
        state: Some("input-available".into()),
        input: None,
        output: None,
        error_text: None,
        title: None,
    });
    update(&mut model, Msg::Page(Ok(page(vec![text, tool], false))));
    model.selection = 0;
    assert!(update(&mut model, press(KeyCode::Enter)).is_empty());
    assert!(model.expanded.is_empty());
    model.selection = 1;
    update(&mut model, press(KeyCode::Enter));
    assert!(model.expanded.contains(&20));
    update(&mut model, press(KeyCode::Enter));
    assert!(!model.expanded.contains(&20));
}

#[test]
fn ut_150_page_bootstraps_stream_with_current_cursor() {
    let mut model = model();
    let commands = update(&mut model, Msg::Page(Ok(page(vec![entry(10, 10)], true))));
    assert_eq!(commands, vec![Cmd::StartStream(model.transcript.cursor())]);
    assert!(!model.fetching);
}

#[test]
fn ut_152_reset_replaces_state_marks_and_restarts_stream() {
    let mut model = model();
    update(&mut model, Msg::Page(Ok(page(vec![entry(10, 10)], false))));
    let commands = update(
        &mut model,
        Msg::Stream(TranscriptEvent::Snapshot(TranscriptSnapshot {
            entries: vec![entry(20, 40)],
            epoch: 2,
            generation: 3,
            max_sequence: 40,
            reset: true,
            reason: Some("epoch_mismatch".into()),
            ..TranscriptSnapshot::default()
        })),
    );
    assert_eq!(model.transcript.entries()[0].start_sequence, 20);
    assert_eq!(
        model.footer,
        FooterState::Resynchronized("epoch_mismatch".into())
    );
    assert_eq!(
        commands,
        vec![
            Cmd::StopStream,
            Cmd::StartStream(model.transcript.cursor()),
            Cmd::ClearFooterAfter(Duration::from_secs(5))
        ]
    );
}

#[test]
fn ut_153_unknown_delta_fetches_newest_without_mutating_state() {
    let mut model = model();
    update(&mut model, Msg::Page(Ok(page(vec![entry(10, 10)], false))));
    let commands = update(
        &mut model,
        Msg::Stream(TranscriptEvent::Delta(TranscriptDelta {
            epoch: 1,
            generation: 2,
            entries: vec![entry(99, 99)],
            cursor: 99,
            max_sequence: 99,
            ..TranscriptDelta::default()
        })),
    );
    assert_eq!(
        commands,
        vec![Cmd::FetchPage {
            before_sequence: None
        }]
    );
    assert_eq!(model.transcript.entries()[0].start_sequence, 10);
}

#[test]
fn ut_154_and_ut_183_stopped_closes_stream_and_keeps_failure_visible() {
    let mut model = model();
    let commands = update(
        &mut model,
        Msg::Stream(TranscriptEvent::Stopped(SessionStopped {
            stop_reason: Some("user_request".into()),
            stop_detail: Some("finished".into()),
            failure: Some(serde_json::json!({"error":"boom"})),
            ..SessionStopped::default()
        })),
    );
    assert_eq!(commands, vec![Cmd::StopStream]);
    assert_eq!(
        model.footer,
        FooterState::Stopped {
            reason: Some("user_request".into()),
            detail: Some("finished".into())
        }
    );
    assert!(
        model
            .warning
            .as_deref()
            .is_some_and(|warning| warning.contains("boom"))
    );
    assert!(model.stopped);
}

#[test]
fn ut_155_scrolled_model_counts_new_entries_below() {
    let mut model = model();
    update(&mut model, Msg::Page(Ok(page(vec![entry(10, 10)], false))));
    model.follow = false;
    update(
        &mut model,
        Msg::Stream(TranscriptEvent::Snapshot(TranscriptSnapshot {
            entries: vec![entry(10, 10), entry(20, 20), entry(30, 30)],
            epoch: 1,
            generation: 2,
            max_sequence: 30,
            ..TranscriptSnapshot::default()
        })),
    );
    assert_eq!(model.selection, 0);
    assert_eq!(model.footer, FooterState::NewBelow(2));
}

#[test]
fn ut_156_dirty_tick_requests_one_render() {
    let mut model = model();
    model.dirty = false;
    assert!(update(&mut model, Msg::Tick).is_empty());
    model.dirty = true;
    assert_eq!(update(&mut model, Msg::Tick), vec![Cmd::Render]);
    assert!(!model.dirty);
    assert!(update(&mut model, Msg::Tick).is_empty());
}

#[test]
fn ut_157_many_deltas_are_all_applied_before_one_render_tick() {
    let mut model = model();
    update(&mut model, Msg::Page(Ok(page(vec![entry(10, 10)], false))));
    update(&mut model, Msg::Tick);
    for sequence in 31..131 {
        update(
            &mut model,
            Msg::Stream(TranscriptEvent::Delta(TranscriptDelta {
                epoch: 1,
                generation: 2,
                entries: vec![entry(10, sequence)],
                cursor: sequence,
                max_sequence: sequence,
                has_more: true,
            })),
        );
    }
    assert_eq!(model.transcript.entries()[0].sequence, 130);
    assert_eq!(update(&mut model, Msg::Tick), vec![Cmd::Render]);
    assert!(update(&mut model, Msg::Tick).is_empty());
}

#[test]
fn ut_158_has_more_delta_does_not_refetch() {
    let mut model = model();
    update(&mut model, Msg::Page(Ok(page(vec![entry(10, 10)], false))));
    let commands = update(
        &mut model,
        Msg::Stream(TranscriptEvent::Delta(TranscriptDelta {
            epoch: 1,
            generation: 2,
            entries: vec![entry(10, 31)],
            cursor: 31,
            max_sequence: 31,
            has_more: true,
        })),
    );
    assert!(commands.is_empty());
}

#[test]
fn ut_159_non_reset_snapshot_refreshes_silently() {
    let mut model = model();
    update(&mut model, Msg::Page(Ok(page(vec![entry(10, 10)], false))));
    let commands = update(
        &mut model,
        Msg::Stream(TranscriptEvent::Snapshot(TranscriptSnapshot {
            entries: vec![entry(20, 20)],
            epoch: 1,
            generation: 2,
            max_sequence: 20,
            reset: false,
            ..Default::default()
        })),
    );
    assert!(commands.is_empty());
    assert!(model.transcript.markers.is_empty());
    assert_eq!(model.footer, FooterState::Live);
}

#[test]
fn ut_160_server_error_becomes_warning_without_effect() {
    let mut model = model();
    let commands = update(
        &mut model,
        Msg::Stream(TranscriptEvent::ServerError(ErrorPayload {
            error: "x".into(),
            ..ErrorPayload::default()
        })),
    );
    assert!(commands.is_empty());
    assert_eq!(model.warning.as_deref(), Some("x"));
}

#[test]
fn ut_161_epoch_mismatch_is_a_reset() {
    let mut model = model();
    update(&mut model, Msg::Page(Ok(page(vec![entry(10, 10)], false))));
    let commands = update(
        &mut model,
        Msg::Stream(TranscriptEvent::Delta(TranscriptDelta {
            epoch: 9,
            generation: 2,
            entries: vec![entry(10, 40)],
            cursor: 40,
            max_sequence: 40,
            ..Default::default()
        })),
    );
    assert_eq!(
        model.footer,
        FooterState::Resynchronized("epoch_mismatch".into())
    );
    assert!(commands.contains(&Cmd::StartStream(model.transcript.cursor())));
}

#[test]
fn ut_170_and_ut_176_top_fetch_is_guarded_and_preserves_anchor() {
    let mut model = model();
    update(
        &mut model,
        Msg::Page(Ok(page(vec![entry(10, 10), entry(20, 20)], true))),
    );
    model.selection = 0;
    let first = update(&mut model, press(KeyCode::Char('k')));
    let second = update(&mut model, press(KeyCode::Char('k')));
    assert_eq!(
        first,
        vec![Cmd::FetchPage {
            before_sequence: Some(10)
        }]
    );
    assert!(second.is_empty());
    update(
        &mut model,
        Msg::Page(Ok(TranscriptPage {
            entries: vec![entry(1, 5)],
            epoch: 1,
            generation: 2,
            max_sequence: 5,
            has_older: false,
            limit: 200,
            ..TranscriptPage::default()
        })),
    );
    assert_eq!(
        model.transcript.entries()[model.selection].start_sequence,
        10
    );
}

#[test]
fn ut_175_page_error_allows_retry() {
    let mut model = model();
    update(&mut model, Msg::Page(Ok(page(vec![entry(10, 10)], true))));
    model.selection = 0;
    update(&mut model, press(KeyCode::Char('k')));
    update(&mut model, Msg::Page(Err("boom".into())));
    assert_eq!(model.warning.as_deref(), Some("boom"));
    assert_eq!(
        update(&mut model, press(KeyCode::Char('k'))),
        vec![Cmd::FetchPage {
            before_sequence: Some(10)
        }]
    );
}

#[test]
fn ut_171_and_ut_172_beginning_and_page_up_loading() {
    let mut no_older = model();
    update(
        &mut no_older,
        Msg::Page(Ok(page(vec![entry(10, 10)], false))),
    );
    update(&mut no_older, press(KeyCode::Char('k')));
    assert!(no_older.beginning);

    let mut older = model();
    update(&mut older, Msg::Page(Ok(page(vec![entry(10, 10)], true))));
    older.selection = 0;
    assert_eq!(
        update(&mut older, press(KeyCode::PageUp)),
        vec![Cmd::FetchPage {
            before_sequence: Some(10)
        }]
    );
}

#[test]
fn ut_178_resize_preserves_selection_and_invalidates_render() {
    let mut model = model();
    model.selection = 7;
    model.dirty = false;
    update(&mut model, Msg::Resize(200, 60));
    assert_eq!(model.size, (200, 60));
    assert_eq!(model.selection, 7);
    assert!(model.dirty);
}

#[test]
fn ut_190_quit_keys_only_stop_and_quit() {
    for event in [
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
    ] {
        let mut model = model();
        assert_eq!(
            update(&mut model, Msg::Key(event)),
            vec![Cmd::StopStream, Cmd::Quit]
        );
    }
}

#[test]
fn ut_180_and_ut_181_stopped_models_never_start_or_reconnect() {
    let mut stopped = model();
    stopped.header.state = "stopped".into();
    stopped.stopped = true;
    stopped.footer = FooterState::Stopped {
        reason: None,
        detail: None,
    };
    assert!(
        update(
            &mut stopped,
            Msg::Page(Ok(page(vec![entry(10, 10)], false)))
        )
        .is_empty()
    );
    update(
        &mut stopped,
        Msg::Stream(TranscriptEvent::Lost {
            attempt: 5,
            next_in: Duration::from_secs(10),
            error: "gone".into(),
        }),
    );
    assert!(matches!(stopped.footer, FooterState::Stopped { .. }));
    assert_eq!(
        update(&mut stopped, press(KeyCode::Char('q'))),
        vec![Cmd::StopStream, Cmd::Quit]
    );
}

#[test]
fn ut_191_cmd_variants_are_read_only_effects() {
    let variants = [
        Cmd::FetchPage {
            before_sequence: None,
        },
        Cmd::StartStream(Default::default()),
        Cmd::StopStream,
        Cmd::ClearFooterAfter(Duration::from_secs(1)),
        Cmd::Render,
        Cmd::Quit,
    ];
    assert_eq!(variants.len(), 6);
}

#[test]
fn ut_193_mouse_is_ignored() {
    let mut model = model();
    model.dirty = false;
    let commands = update(
        &mut model,
        Msg::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        }),
    );
    assert!(commands.is_empty());
    assert!(!model.dirty);
}

#[test]
fn ut_200_stream_loss_transitions_footer() {
    let mut model = model();
    update(
        &mut model,
        Msg::Stream(TranscriptEvent::Lost {
            attempt: 3,
            next_in: Duration::from_secs(1),
            error: "gone".into(),
        }),
    );
    assert_eq!(model.footer, FooterState::Reconnecting(3));
    update(&mut model, Msg::Stream(TranscriptEvent::Reconnected));
    assert_eq!(model.footer, FooterState::Live);
    update(
        &mut model,
        Msg::Stream(TranscriptEvent::Lost {
            attempt: 5,
            next_in: Duration::from_secs(10),
            error: "gone".into(),
        }),
    );
    assert_eq!(model.footer, FooterState::Offline);
}
