mod api;
mod attention;
mod clarify;
mod composer;
mod detail_run;
mod detail_session;
mod keys;
mod logs;
mod picker;
mod prompt;
mod runs;
mod sessions;
mod stream;
mod timers;

use crate::{app::model::Model, cmd::Cmd, msg::Msg};

pub fn update(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    let commands = match msg {
        Msg::Key(key) => keys::key(model, key),
        Msg::Mouse(_) => Vec::new(),
        Msg::Resize(width, height) => {
            model.size = (width, height);
            let closing_logs = if model.too_small() {
                match &model.overlay {
                    Some(crate::app::Overlay::Logs { scope, .. }) => {
                        Some(crate::cmd::StreamId::Logs(scope.clone()))
                    }
                    _ => None,
                }
            } else {
                None
            };
            if model.too_small() {
                model.overlay = None;
            }
            if let Some(detail) = model.session_detail_mut() {
                detail.view.visible_height = usize::from(height.saturating_sub(6)).max(1);
                detail.view.cache_dirty = true;
            }
            model.dirty = true;
            closing_logs
                .into_iter()
                .map(|stream| {
                    model.active_streams.remove(&stream);
                    Cmd::StopStream(stream)
                })
                .collect()
        }
        Msg::Tick => {
            if model.dirty {
                model.dirty = false;
                vec![Cmd::Render]
            } else {
                Vec::new()
            }
        }
        Msg::Api { request, result } => api::api(model, request, result),
        Msg::Stream { id, event } => stream::stream(model, id, event),
        Msg::Timer(id) => timers::timer(model, id),
        Msg::Quit => keys::quit(model),
    };
    if model
        .session_detail()
        .is_some_and(|detail| detail.view.cache_dirty)
    {
        crate::render_cache::rebuild(model);
    }
    commands
}
