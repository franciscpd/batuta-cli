pub mod cards;
pub mod footer;
pub mod header;
pub mod layout;
pub mod markdown;
pub mod overlays;
pub mod tail;
pub mod toast;
pub mod transcript;

use crate::{
    app::{AppMode, Detail, Model, Panel, StreamStatus},
    cmd::StreamId,
};
use ratatui::{
    Frame,
    style::Modifier,
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
    for (panel, area, title, empty) in [
        (
            Panel::Sessions,
            areas.sessions,
            format!("[1] Sessions · {}", model.settings.preset.agent),
            "no sessions — press n to start one",
        ),
        (
            Panel::Runs,
            areas.runs,
            format!("[2] Deliver runs · {}", model.settings.preset.loop_name),
            "no runs for batuta-deliver — press * for all",
        ),
        (
            Panel::Attention,
            areas.attention,
            format!("[3] Attention ({})", model.attention.len()),
            "nothing needs attention",
        ),
    ] {
        if area.width == 0 || area.height == 0 {
            continue;
        }
        let style = if model.focus == panel {
            model.theme.emphasis
        } else {
            model.theme.muted
        };
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(style);
        let text = match panel {
            Panel::Sessions => {
                let mut text = if model.sessions.items.is_empty() {
                    empty.to_owned()
                } else {
                    model
                        .sessions
                        .items
                        .iter()
                        .map(|row| format!("{} {}", row.agent, row.name.as_deref().unwrap_or("—")))
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                if matches!(
                    model.stream_status.get(&StreamId::Catalog),
                    Some(StreamStatus::Stale | StreamStatus::Fatal(_))
                ) {
                    text.push_str("\ncatalog: stale");
                }
                text
            }
            Panel::Runs => {
                if model.runs.items.is_empty() {
                    empty.to_owned()
                } else {
                    model
                        .runs
                        .items
                        .iter()
                        .map(|row| format!("{} {}", row.status, row.id))
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            }
            Panel::Attention => {
                if model.attention.is_empty() {
                    empty.to_owned()
                } else {
                    model
                        .attention
                        .iter()
                        .map(|item| item.title.clone())
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            }
            Panel::Detail => String::new(),
        };
        let mut paragraph = Paragraph::new(text).block(block);
        if header::offline(model) {
            paragraph = paragraph.style(model.theme.muted.add_modifier(Modifier::DIM));
        }
        frame.render_widget(paragraph, area);
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
        Detail::Session(_) => transcript::render(model, frame, inner),
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
