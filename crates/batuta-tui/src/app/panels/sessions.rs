use crate::{
    app::model::{Model, SessionRow},
    cmd::{Cmd, Request},
};
use compozy_client::types::SessionPage;

pub fn request(model: &mut Model) -> Option<Cmd> {
    let workspace = model.workspace.as_ref()?.id.clone();
    let preset_only = model.sessions_preset_only;
    let agent = preset_only.then(|| model.settings.preset.agent.clone());
    let session_type = preset_only.then(|| "user".to_owned());
    let limit = model.settings.ui.sessions_limit;
    Some(Cmd::Get(model.allocate(|id| Request::Sessions {
        id,
        workspace,
        agent,
        session_type,
        limit,
    })))
}

pub fn apply(model: &mut Model, page: SessionPage) {
    let selected_id = model.sessions.selected().map(|row| row.id.clone());
    model.sessions_has_more = page.page.has_more;
    model.sessions_unfiltered = page.sessions.iter().map(SessionRow::from).collect();
    refilter(model, selected_id.as_deref());
}

pub fn refilter(model: &mut Model, selected_id: Option<&str>) {
    let needle = model.sessions.filter.to_lowercase();
    let items = model
        .sessions_unfiltered
        .iter()
        .filter(|row| {
            needle.is_empty()
                || row.id.to_lowercase().contains(&needle)
                || row.agent.to_lowercase().contains(&needle)
                || row
                    .name
                    .as_deref()
                    .is_some_and(|name| name.to_lowercase().contains(&needle))
        })
        .cloned()
        .collect::<Vec<_>>();
    model.sessions.set_items(items);
    if let Some(id) = selected_id
        && let Some(index) = model.sessions.items.iter().position(|row| row.id == id)
    {
        model.sessions.selected = Some(index);
    }
}

pub fn glyph(row: &SessionRow) -> &'static str {
    match row.badge.as_deref().unwrap_or(&row.state) {
        "running" | "active" => "●",
        "stopped" => "■",
        "failed" | "unhealthy" | "error" => "!",
        _ => "○",
    }
}

pub fn relative(timestamp: Option<&compozy_client::types::Timestamp>, now: i64) -> String {
    let Some(then) = timestamp.and_then(|value| value.parsed()) else {
        return "—".into();
    };
    let seconds = now.saturating_sub(then.unix_timestamp()).max(0);
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3_600 {
        format!("{}m", seconds / 60)
    } else if seconds < 86_400 {
        format!("{}h", seconds / 3_600)
    } else {
        format!("{}d", seconds / 86_400)
    }
}
