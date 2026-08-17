use batuta_tui::transcript::{Applied, TranscriptState};
use compozy_client::types::{
    Entry, TranscriptDelta, TranscriptPage, TranscriptSnapshot, UiMessage,
};

fn entry(start_sequence: i64, sequence: i64) -> Entry {
    Entry {
        start_sequence,
        sequence,
        message: UiMessage {
            id: format!("message-{start_sequence}"),
            ..UiMessage::default()
        },
    }
}

fn page(entries: Vec<Entry>) -> TranscriptPage {
    TranscriptPage {
        entries,
        epoch: 2,
        generation: 3,
        max_sequence: 30,
        has_older: true,
        next_before_sequence: Some(10),
        limit: 200,
    }
}

#[test]
fn ut_150_snapshot_sets_entries_and_cursor() {
    let mut state = TranscriptState::default();
    let applied = state.apply_snapshot(TranscriptSnapshot {
        entries: vec![entry(20, 25), entry(10, 15)],
        epoch: 2,
        generation: 3,
        max_sequence: 30,
        has_older: true,
        next_before_sequence: Some(10),
        ..TranscriptSnapshot::default()
    });

    assert_eq!(applied, Applied::Ok);
    assert_eq!(
        state
            .entries()
            .iter()
            .map(|entry| entry.start_sequence)
            .collect::<Vec<_>>(),
        vec![10, 20]
    );
    assert_eq!(state.cursor().after_sequence, 30);
    assert_eq!(state.cursor().epoch, 2);
    assert_eq!(state.cursor().generation, 3);
}

#[test]
fn ut_151_known_delta_replaces_entry_by_start_sequence() {
    let mut state = TranscriptState::default();
    state.prepend_page(page(vec![entry(10, 15)]));

    let applied = state.apply_delta(TranscriptDelta {
        epoch: 2,
        generation: 3,
        entries: vec![entry(10, 29)],
        cursor: 29,
        max_sequence: 30,
        ..TranscriptDelta::default()
    });

    assert_eq!(applied, Applied::Ok);
    assert_eq!(state.entries()[0].sequence, 29);
    assert_eq!(state.cursor().after_sequence, 30);
}

#[test]
fn ut_173_prepend_page_orders_entries_and_updates_paging() {
    let mut state = TranscriptState::default();
    state.prepend_page(page(vec![entry(20, 25)]));
    state.prepend_page(TranscriptPage {
        entries: vec![entry(5, 8), entry(10, 15)],
        epoch: 2,
        generation: 3,
        max_sequence: 15,
        has_older: false,
        next_before_sequence: None,
        limit: 200,
    });

    assert_eq!(
        state
            .entries()
            .iter()
            .map(|entry| entry.start_sequence)
            .collect::<Vec<_>>(),
        vec![5, 10, 20]
    );
    assert!(!state.has_older);
    assert_eq!(state.next_before_sequence, None);
    assert_eq!(state.max_sequence, 30);
}

#[test]
fn ut_174_unknown_start_and_fence_mismatch_do_not_partially_apply() {
    let mut state = TranscriptState::default();
    state.prepend_page(page(vec![entry(10, 15)]));

    let unknown = state.apply_delta(TranscriptDelta {
        epoch: 2,
        generation: 3,
        entries: vec![entry(99, 31)],
        cursor: 31,
        max_sequence: 31,
        ..TranscriptDelta::default()
    });
    assert_eq!(unknown, Applied::UnknownStart(99));
    assert_eq!(state.entries().len(), 1);

    let mismatch = state.apply_delta(TranscriptDelta {
        epoch: 4,
        generation: 3,
        entries: vec![entry(10, 32)],
        cursor: 32,
        max_sequence: 32,
        ..TranscriptDelta::default()
    });
    assert_eq!(mismatch, Applied::FenceMismatch);
    assert_eq!(state.entries()[0].sequence, 15);
}
