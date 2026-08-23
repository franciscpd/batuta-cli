use crate::{
    app::model::{FooterState, Model, StreamStatus},
    cmd::{Cmd, StreamId},
    msg::AnyStreamEvent,
    transcript::{Applied, PresentationRow},
};
use compozy_client::{Error, StreamEvent, TranscriptEvent, types::TranscriptSnapshot};

pub(super) fn stream(model: &mut Model, id: StreamId, event: AnyStreamEvent) -> Vec<Cmd> {
    if !model.active_streams.contains(&id) {
        return Vec::new();
    }
    if event.is_lost() {
        model.stream_status.insert(id.clone(), StreamStatus::Stale);
    } else if event.is_live() {
        model.stream_status.insert(id.clone(), StreamStatus::Live);
    }
    let commands = match (id.clone(), event) {
        (StreamId::Transcript(session), AnyStreamEvent::Transcript(event)) => {
            let permission_delta = match &event {
                TranscriptEvent::Delta(delta) => delta.entries.iter().any(|entry| {
                    crate::app::panels::attention::permission_from_delta(&entry.message.parts)
                }),
                TranscriptEvent::Snapshot(snapshot) => snapshot.entries.iter().any(|entry| {
                    crate::app::panels::attention::permission_from_delta(&entry.message.parts)
                }),
                _ => false,
            };
            let commands = transcript(model, &session, event);
            if let Some(detail) = model.session_detail() {
                let cursor = detail.transcript.cursor();
                model.stream_cursors.insert(
                    StreamId::Transcript(session),
                    format!(
                        "{}:{}:{}",
                        cursor.epoch, cursor.generation, cursor.after_sequence
                    ),
                );
            }
            if permission_delta {
                crate::app::panels::attention::sync_open_detail(model);
                let mut all = commands;
                all.extend(crate::app::panels::attention::rebuild(model));
                all.extend(super::attention::refresh(model));
                all
            } else {
                commands
            }
        }
        (StreamId::Catalog, AnyStreamEvent::Catalog(StreamEvent::Event(event))) => {
            let active = model.workspace.as_ref().map(|value| value.id.as_str());
            if active != Some(event.workspace_id.as_str()) || model.catalog_debounce_armed {
                Vec::new()
            } else {
                model.catalog_debounce_armed = true;
                vec![Cmd::After(
                    std::time::Duration::from_millis(300),
                    crate::cmd::TimerId::CatalogDebounce,
                )]
            }
        }
        (StreamId::RunEvents(run), AnyStreamEvent::Run(StreamEvent::Event(event))) => {
            let seq = event.seq;
            let mut terminal = false;
            if let crate::app::model::Detail::Run(detail) = &mut model.detail
                && detail.run_id == run
            {
                if detail.events.iter().any(|item| item.seq == seq) {
                    return Vec::new();
                }
                if event.kind == "status_changed"
                    && let Some(status) = event
                        .payload
                        .get("status")
                        .and_then(serde_json::Value::as_str)
                {
                    terminal = crate::app::panels::runs::terminal(status);
                    if let Some(value) = &mut detail.run {
                        value.run.status = status.to_owned();
                    }
                }
                detail.events.push(event);
                detail.events.sort_by_key(|item| item.seq);
                model.dirty = true;
            }
            model
                .stream_cursors
                .insert(StreamId::RunEvents(run.clone()), seq.to_string());
            if terminal {
                let stream = StreamId::RunEvents(run);
                model.active_streams.remove(&stream);
                vec![Cmd::StopStream(stream)]
            } else {
                Vec::new()
            }
        }
        (StreamId::Logs(scope), AnyStreamEvent::Logs(StreamEvent::Event(event))) => {
            model.stream_cursors.insert(
                StreamId::Logs(scope.clone()),
                format!("{}|{}", event.timestamp.raw(), event.id),
            );
            super::logs::push(model, &scope, event);
            Vec::new()
        }
        (
            StreamId::Logs(scope),
            AnyStreamEvent::Logs(StreamEvent::Fatal(Error::Daemon { status: 400, .. })),
        ) => super::logs::cursor_reset(model, scope),
        _ => Vec::new(),
    };
    model.dirty = true;
    commands
}

fn transcript(model: &mut Model, session: &str, event: TranscriptEvent) -> Vec<Cmd> {
    let workspace = model
        .workspace
        .as_ref()
        .map(|value| value.id.clone())
        .unwrap_or_default();
    let Some(detail) = model
        .session_detail_mut()
        .filter(|detail| detail.session.id == session)
    else {
        return Vec::new();
    };
    match event {
        TranscriptEvent::Snapshot(snapshot) => apply_snapshot(detail, snapshot),
        TranscriptEvent::Delta(delta) => {
            let raw_updates = delta.entries.len();
            match detail.transcript.apply_delta(delta.clone()) {
                Applied::Ok => {
                    detail.view.cache_dirty = true;
                    super::search::recompute_search(detail);
                    if detail.view.follow {
                        detail.view.selection = detail
                            .transcript
                            .presentation_rows(detail.view.raw_debug)
                            .len()
                            .saturating_sub(1);
                    } else if raw_updates > 0 {
                        let previous = match detail.view.footer {
                            FooterState::NewBelow(count) => count,
                            _ => 0,
                        };
                        detail.view.footer =
                            FooterState::NewBelow(previous.saturating_add(raw_updates));
                    }
                    Vec::new()
                }
                Applied::UnknownStart(_) => {
                    if detail.view.fetching {
                        Vec::new()
                    } else {
                        detail.view.fetching = true;
                        let session = session.to_owned();
                        let request = model.allocate(|id| crate::cmd::Request::TranscriptPage {
                            id,
                            workspace,
                            session,
                            before_sequence: None,
                        });
                        vec![Cmd::Get(request)]
                    }
                }
                Applied::FenceMismatch => apply_snapshot(
                    detail,
                    TranscriptSnapshot {
                        epoch: delta.epoch,
                        generation: delta.generation,
                        entries: delta.entries,
                        max_sequence: delta.max_sequence.max(delta.cursor),
                        reset: true,
                        reason: Some("epoch_mismatch".into()),
                        ..TranscriptSnapshot::default()
                    },
                ),
            }
        }
        TranscriptEvent::Stopped(stopped) => {
            detail.view.stopped = true;
            detail.stream = StreamStatus::Stopped;
            detail.session.state = "stopped".into();
            detail.view.footer = FooterState::Stopped {
                reason: stopped.stop_reason,
                detail: stopped.stop_detail,
            };
            model
                .active_streams
                .remove(&StreamId::Transcript(session.to_owned()));
            vec![Cmd::StopStream(StreamId::Transcript(session.to_owned()))]
        }
        TranscriptEvent::Lost {
            attempt,
            offline_after,
            ..
        } => {
            detail.stream = StreamStatus::Reconnecting(attempt);
            detail.view.footer = if attempt >= offline_after {
                FooterState::Offline
            } else {
                FooterState::Reconnecting(attempt)
            };
            Vec::new()
        }
        TranscriptEvent::Reconnected => {
            detail.stream = StreamStatus::Live;
            detail.view.footer = FooterState::Live;
            Vec::new()
        }
        TranscriptEvent::ServerError(error) => {
            detail.view.warning = Some(error.error);
            Vec::new()
        }
        TranscriptEvent::Fatal(error) => {
            detail.stream = StreamStatus::Fatal(error.to_string());
            detail.view.footer = FooterState::Fatal(error.to_string());
            Vec::new()
        }
    }
}

fn apply_snapshot(
    detail: &mut crate::app::model::SessionDetail,
    snapshot: TranscriptSnapshot,
) -> Vec<Cmd> {
    let reset = snapshot.reset;
    let reason = snapshot.reason.clone().unwrap_or_else(|| "reset".into());
    let old_len = detail.transcript.len();
    let selected_source = (!detail.view.follow)
        .then(|| selected_source_start_sequence(detail))
        .flatten();
    detail.transcript.apply_snapshot(snapshot);
    detail.view.cache_dirty = true;
    super::search::recompute_search(detail);
    if detail.view.follow {
        detail.view.selection = detail
            .transcript
            .presentation_rows(detail.view.raw_debug)
            .len()
            .saturating_sub(1);
    } else {
        let added = detail.transcript.len().saturating_sub(old_len);
        if added > 0 {
            detail.view.footer = FooterState::NewBelow(added);
        }
        if let Some(start_sequence) = selected_source {
            remap_selection(detail, start_sequence);
        }
    }
    if reset {
        detail.view.footer = FooterState::Resynchronized(reason);
        vec![Cmd::After(
            std::time::Duration::from_secs(5),
            crate::cmd::TimerId::ToastExpiry,
        )]
    } else {
        Vec::new()
    }
}

fn selected_source_start_sequence(detail: &crate::app::model::SessionDetail) -> Option<i64> {
    let entries = detail.transcript.entries();
    let rows = detail.transcript.presentation_rows(detail.view.raw_debug);
    let row = rows.get(detail.view.selection)?;
    detail
        .view
        .selected_source_start_sequence
        .filter(|start_sequence| row_contains_start_sequence(row, &entries, *start_sequence))
        .or_else(|| {
            entries
                .get(row.first_entry_index())
                .map(|entry| entry.start_sequence)
        })
}

fn remap_selection(detail: &mut crate::app::model::SessionDetail, start_sequence: i64) {
    let entries = detail.transcript.entries();
    let rows = detail.transcript.presentation_rows(detail.view.raw_debug);
    let selected = rows
        .iter()
        .position(|row| row_contains_start_sequence(row, &entries, start_sequence))
        .or_else(|| {
            rows.iter()
                .enumerate()
                .filter_map(|(index, row)| {
                    row_start_sequences(row, &entries)
                        .map(|sequence| sequence.abs_diff(start_sequence))
                        .min()
                        .map(|distance| (index, distance))
                })
                .min_by_key(|(_, distance)| *distance)
                .map(|(index, _)| index)
        });
    if let Some(selection) = selected {
        detail.view.selection = selection;
        detail.view.selected_source_start_sequence = rows.get(selection).and_then(|row| {
            row_start_sequences(row, &entries)
                .min_by_key(|sequence| sequence.abs_diff(start_sequence))
        });
    } else {
        detail.view.selection = 0;
        detail.view.selected_source_start_sequence = None;
    }
}

fn row_contains_start_sequence(
    row: &PresentationRow,
    entries: &[&compozy_client::types::Entry],
    start_sequence: i64,
) -> bool {
    row_start_sequences(row, entries).any(|sequence| sequence == start_sequence)
}

fn row_start_sequences<'a>(
    row: &'a PresentationRow,
    entries: &'a [&'a compozy_client::types::Entry],
) -> impl Iterator<Item = i64> + 'a {
    let indexes: &[usize] = match row {
        PresentationRow::Entry { entry_index } => std::slice::from_ref(entry_index),
        PresentationRow::Group { entry_indexes, .. } => entry_indexes,
    };
    indexes
        .iter()
        .filter_map(|index| entries.get(*index).map(|entry| entry.start_sequence))
}
