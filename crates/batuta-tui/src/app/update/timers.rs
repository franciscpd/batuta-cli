use crate::{
    app::model::{Model, Panel},
    cmd::{Cmd, Request, TimerId},
};

pub(super) fn timer(model: &mut Model, id: TimerId) -> Vec<Cmd> {
    match id {
        TimerId::ToastExpiry => {
            if model.toast.as_ref().is_some_and(|toast| !toast.sticky) {
                model.toast = None;
                model.dirty = true;
            }
            if let Some(detail) = model.session_detail_mut()
                && matches!(
                    detail.view.footer,
                    crate::app::model::FooterState::Resynchronized(_)
                )
            {
                detail.view.footer = crate::app::model::FooterState::Live;
                model.dirty = true;
            }
            Vec::new()
        }
        TimerId::QuitGuard => {
            model.quit_guard = false;
            if model
                .toast
                .as_ref()
                .is_some_and(|toast| toast.text.starts_with("press Ctrl+C"))
            {
                model.toast = None;
                model.dirty = true;
            }
            Vec::new()
        }
        TimerId::DetailSwitchDebounce => match model.last_list_focus {
            Panel::Sessions => super::keys::open_session(model),
            Panel::Runs => super::keys::open_run(model),
            _ => Vec::new(),
        },
        TimerId::StatusPoll => {
            let request = model.allocate(|request_id| Request::Status { id: request_id });
            vec![
                Cmd::Get(request),
                Cmd::After(std::time::Duration::from_secs(30), TimerId::StatusPoll),
            ]
        }
        TimerId::AttentionPoll => {
            let mut commands = super::attention::refresh(model);
            commands.push(Cmd::After(
                std::time::Duration::from_secs(30),
                TimerId::AttentionPoll,
            ));
            commands
        }
        TimerId::CatalogDebounce => {
            model.catalog_debounce_armed = false;
            let mut commands = super::sessions::refresh(model);
            commands.extend(super::attention::refresh(model));
            commands
        }
        TimerId::CatalogPoll => {
            if !model.catalog_polling {
                return Vec::new();
            }
            let mut commands = super::sessions::refresh(model);
            commands.extend(super::attention::refresh(model));
            commands.push(Cmd::After(
                std::time::Duration::from_secs(10),
                TimerId::CatalogPoll,
            ));
            commands
        }
        TimerId::RunsPoll => super::runs::refresh(model),
    }
}
