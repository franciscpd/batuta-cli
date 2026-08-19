use crate::{
    app::{Model, Panel, panels::sessions},
    cmd::StreamId,
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};

pub fn render(model: &Model, frame: &mut Frame<'_>, area: Rect, offline: bool) {
    let scope = if model.sessions_all_agents {
        "all agents"
    } else {
        &model.settings.preset.agent
    };
    let suffix = if model.sessions.filter.is_empty() {
        String::new()
    } else {
        format!(" · /{}", model.sessions.filter)
    };
    let border = if model.focus == Panel::Sessions {
        model.theme.emphasis
    } else {
        model.theme.muted
    };
    let lines = if model.sessions.items.is_empty() {
        vec![Line::from(if model.sessions.filter.is_empty() {
            "no sessions — press n to start one"
        } else {
            "no match"
        })]
    } else {
        model
            .sessions
            .items
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let selected = model.sessions.selected == Some(index);
                let glyph = sessions::glyph(row);
                let status = match glyph {
                    "●" => model.theme.success,
                    "!" => model.theme.error,
                    "■" => model.theme.muted,
                    _ => model.theme.default,
                };
                Line::from(vec![
                    Span::styled(
                        if selected { "> " } else { "  " },
                        if selected {
                            model.theme.selection
                        } else {
                            model.theme.default
                        },
                    ),
                    Span::styled(format!("{glyph} "), status),
                    Span::raw(format!(
                        "{}  {}  ",
                        row.agent,
                        row.name.as_deref().unwrap_or("—")
                    )),
                    Span::styled(
                        sessions::relative(row.last_activity_at.as_ref(), model.now_unix),
                        model.theme.muted,
                    ),
                ])
            })
            .collect()
    };
    let mut text = Text::from(lines);
    if model.sessions_has_more {
        text.lines.push(Line::styled(
            format!("more (limit {})", model.settings.ui.sessions_limit),
            model.theme.muted,
        ));
    }
    if matches!(
        model.stream_status.get(&StreamId::Catalog),
        Some(crate::app::StreamStatus::Stale | crate::app::StreamStatus::Fatal(_))
    ) {
        text.lines
            .push(Line::styled("catalog: stale", model.theme.muted));
    }
    let title = format!("[1] Sessions · {scope}{suffix}");
    let mut paragraph = Paragraph::new(text).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border),
    );
    if offline {
        paragraph = paragraph.style(model.theme.muted.add_modifier(Modifier::DIM));
    }
    frame.render_widget(paragraph, area);
}
