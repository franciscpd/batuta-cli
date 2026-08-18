pub mod attention;
pub mod cards;
pub mod chooser;
pub mod clarify;
pub mod composer;
pub mod footer;
pub mod header;
pub mod layout;
pub mod markdown;
pub mod overlays;
pub mod runs;
pub mod session;
pub mod sessions;
pub mod tail;
pub mod toast;
pub mod transcript;

use crate::app::{AppMode, Detail, Model, Panel};
use ratatui::{
    Frame,
    widgets::{Block, Borders, Paragraph},
};

pub fn view(model: &Model, frame: &mut Frame<'_>) {
    if model.mode == AppMode::TailOnly {
        return tail::view(model, frame);
    }
    let areas = layout::areas(frame.area(), model);
    if areas.mode == layout::LayoutMode::TooSmall {
        let text = format!(
            "batuta needs at least 80×24 (now {}×{})\nresize the terminal — q quits",
            frame.area().width,
            frame.area().height
        );
        frame.render_widget(
            Paragraph::new(text).centered().style(model.theme.warning),
            frame.area(),
        );
        return;
    }
    header::render(model, frame, areas.header);
    let offline = header::offline(model);
    if areas.sessions.width > 0 {
        sessions::render(model, frame, areas.sessions, offline);
    }
    if areas.runs.width > 0 {
        runs::render(model, frame, areas.runs, offline);
    }
    if areas.attention.width > 0 {
        attention::render(model, frame, areas.attention, offline);
    }
    render_detail(model, frame, areas.detail);
    footer::render(model, frame, areas.footer);
    overlays::render(model, frame);
    toast::render(model, frame);
}

fn render_detail(model: &Model, frame: &mut Frame<'_>, area: ratatui::layout::Rect) {
    let title = match &model.detail {
        Detail::Session(detail) => format!(
            "[4] Session · {} · {}",
            detail.session.id, detail.session.agent_name
        ),
        Detail::Run(detail) => format!("[4] Run · {}", detail.run_id),
        Detail::Empty => "[4] Detail".into(),
    };
    let style = if model.focus == Panel::Detail {
        model.theme.emphasis
    } else {
        model.theme.muted
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(style);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    match &model.detail {
        Detail::Session(_) => session::render(model, frame, inner),
        Detail::Run(detail) => {
            let text = if detail.events.is_empty() {
                "no run events".into()
            } else {
                detail
                    .events
                    .iter()
                    .map(|event| format!("{} {}", event.seq, event.kind))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            frame.render_widget(Paragraph::new(text), inner)
        }
        Detail::Empty => frame.render_widget(
            Paragraph::new("select a session or run").style(model.theme.muted),
            inner,
        ),
    }
}
