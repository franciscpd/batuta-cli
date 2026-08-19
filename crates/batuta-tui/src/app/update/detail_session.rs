use crate::{
    app::{Confirm, Model},
    cmd::{Cmd, Request},
};

pub(super) fn cancel_key(model: &mut Model) -> Vec<Cmd> {
    if !super::prompt::busy(model) {
        model.set_sticky_toast("no turn in progress");
        return Vec::new();
    }
    model.session_detail_mut().unwrap().confirm = Some(Confirm {
        prompt: "cancel the current turn? Enter/Esc".into(),
    });
    model.dirty = true;
    Vec::new()
}

pub(super) fn confirm_cancel(model: &mut Model) -> Vec<Cmd> {
    let Some(workspace) = model
        .workspace
        .as_ref()
        .map(|workspace| workspace.id.clone())
    else {
        return Vec::new();
    };
    let Some(session) = model
        .session_detail()
        .map(|detail| detail.session.id.clone())
    else {
        return Vec::new();
    };
    if model.refuse_write("cancel turn") {
        return Vec::new();
    }
    model.session_detail_mut().unwrap().confirm = None;
    let request = model.allocate(|id| Request::CancelPrompt {
        id,
        workspace,
        session,
    });
    vec![Cmd::Post(request)]
}

pub(super) fn reopen_if_active(model: &mut Model) -> Vec<Cmd> {
    let Some((session, stopped)) = model.session_detail().map(|detail| {
        (
            detail.session.id.clone(),
            detail.stream == crate::app::StreamStatus::Stopped || detail.view.stopped,
        )
    }) else {
        return Vec::new();
    };
    let active = model
        .sessions_unfiltered
        .iter()
        .find(|row| row.id == session)
        .is_some_and(|row| row.state != "stopped");
    if !stopped || !active {
        return Vec::new();
    }
    let stream = crate::cmd::StreamId::Transcript(session);
    model.active_streams.insert(stream.clone());
    if let Some(detail) = model.session_detail_mut() {
        detail.view.stopped = false;
        detail.stream = crate::app::StreamStatus::Live;
        detail.view.footer = crate::app::FooterState::Live;
        detail.session.state = "active".into();
    }
    vec![Cmd::StartStream(stream)]
}
