use crate::{
    app::model::{
        AttentionItem, Detail, Model, PendingKind, RunRow, SessionDetail, SessionRow, StreamStatus,
        WorkspaceRef, page_into_detail,
    },
    cmd::{Cmd, Request, RequestId, StreamId},
    msg::{ApiResponse, ApiResult},
};

pub(super) fn api(model: &mut Model, id: RequestId, result: ApiResult) -> Vec<Cmd> {
    let Some(PendingKind::Request(request)) = model.pending.remove(&id) else {
        return Vec::new();
    };
    match result {
        Err(error) => {
            if matches!(request, Request::Status { .. }) {
                model.daemon.poll_ok = false;
            } else {
                model.set_sticky_toast(error);
            }
            model.dirty = true;
            Vec::new()
        }
        Ok(response) => {
            model.daemon.poll_ok = true;
            apply(model, request, response)
        }
    }
}

fn apply(model: &mut Model, request: Request, response: ApiResponse) -> Vec<Cmd> {
    match (request, response) {
        (Request::Status { .. }, ApiResponse::Status(status)) => {
            model.daemon.status = status.daemon.status;
            model.daemon.version = status.daemon.version;
            model.dirty = true;
            Vec::new()
        }
        (Request::Workspaces { .. }, ApiResponse::Workspaces(items)) => {
            if let Some(crate::app::model::Overlay::WorkspacePicker {
                items: target,
                selected,
                ..
            }) = &mut model.overlay
            {
                *target = items
                    .into_iter()
                    .map(|item| WorkspaceRef {
                        id: item.id,
                        name: item.name,
                        root_dir: item.root_dir,
                    })
                    .collect();
                *selected = (!target.is_empty()).then_some(0);
            }
            model.dirty = true;
            Vec::new()
        }
        (Request::Sessions { .. }, ApiResponse::Sessions(page)) => {
            model
                .sessions
                .set_items(page.sessions.iter().map(SessionRow::from).collect());
            model.dirty = true;
            Vec::new()
        }
        (Request::Runs { .. }, ApiResponse::Runs(page)) => {
            model.runs.set_items(
                page.runs
                    .into_iter()
                    .map(|run| RunRow {
                        id: run.id,
                        loop_name: run.loop_name,
                        status: run.status,
                        parent_id: run.parent_loop_run_id,
                    })
                    .collect(),
            );
            model.dirty = true;
            Vec::new()
        }
        (Request::Overview { .. }, ApiResponse::Overview(overview)) => {
            model.attention = overview
                .attention
                .items
                .into_iter()
                .map(|item| AttentionItem {
                    title: item.title,
                    detail: item.detail,
                    session_id: (!item.session_id.is_empty()).then_some(item.session_id),
                    run_id: (!item.run_id.is_empty()).then_some(item.run_id),
                })
                .collect();
            model.dirty = true;
            Vec::new()
        }
        (Request::Session { session: id, .. }, ApiResponse::Session(session)) => {
            if let Some(detail) = model
                .session_detail_mut()
                .filter(|detail| detail.session.id == id)
            {
                detail.session = session;
            } else {
                model.detail = Detail::Session(Box::new(SessionDetail::new(session)));
            }
            model.focus = crate::app::model::Panel::Detail;
            model.dirty = true;
            Vec::new()
        }
        (Request::TranscriptPage { session, .. }, ApiResponse::TranscriptPage(page)) => {
            if model.session_detail().is_some() {
                let (cursor, stopped) = {
                    let detail = model.session_detail_mut().expect("checked session detail");
                    page_into_detail(detail, *page);
                    (detail.transcript.cursor(), detail.view.stopped)
                };
                model.stream_cursors.insert(
                    StreamId::Transcript(session.clone()),
                    format!(
                        "{}:{}:{}",
                        cursor.epoch, cursor.generation, cursor.after_sequence
                    ),
                );
                model
                    .active_streams
                    .insert(StreamId::Transcript(session.clone()));
                model.dirty = true;
                if stopped {
                    model.active_streams.remove(&StreamId::Transcript(session));
                    Vec::new()
                } else {
                    vec![Cmd::StartStream(StreamId::Transcript(session))]
                }
            } else {
                Vec::new()
            }
        }
        (Request::Run { run, .. }, ApiResponse::Run(value)) => {
            if let Detail::Run(detail) = &mut model.detail
                && detail.run_id == run
            {
                detail.run = Some(*value);
                detail.stream = StreamStatus::Live;
                model.dirty = true;
            }
            Vec::new()
        }
        (Request::Prompt { .. }, ApiResponse::Prompt(_)) => model.set_success_toast("sent"),
        (Request::CreateSession { .. }, ApiResponse::SessionCreated(session)) => {
            model.detail = Detail::Session(Box::new(SessionDetail::new(session)));
            model.set_success_toast("session created")
        }
        (_, _) => Vec::new(),
    }
}
