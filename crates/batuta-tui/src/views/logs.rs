use crate::{
    app::{Model, Overlay},
    cmd::LogScope,
};
use ratatui::text::{Line, Span};

pub fn content(model: &Model) -> Option<(String, Vec<Line<'static>>, usize)> {
    let Overlay::Logs {
        scope,
        error_only,
        events,
        selection,
        cursor_reset,
    } = model.overlay.as_ref()?
    else {
        return None;
    };
    let scope = match scope {
        LogScope::Workspace => "workspace".to_owned(),
        LogScope::Session(id) => format!("session {id}"),
        LogScope::Run(id) => format!("run {id}"),
    };
    let mut title = format!("logs · {scope} · live");
    if *error_only {
        title.push_str(" · errors");
    }
    let mut lines = Vec::new();
    if *cursor_reset {
        lines.push(Line::from(Span::styled("cursor reset", model.theme.muted)));
    }
    for event in events {
        let error = matches!(event.outcome.as_str(), "error" | "failed" | "failure")
            || event.type_.contains("error");
        let glyph = if error { "!" } else { "○" };
        let time = event
            .timestamp
            .raw()
            .get(11..19)
            .unwrap_or(event.timestamp.raw());
        let text = format!(
            "{time}  {}  {glyph}  {}  {}",
            event.type_,
            if event.agent_name.is_empty() {
                "—"
            } else {
                &event.agent_name
            },
            event.summary
        );
        lines.push(Line::from(Span::styled(
            text,
            if error {
                model.theme.error
            } else {
                model.theme.default
            },
        )));
    }
    Some((title, lines, *selection))
}
