use crate::app::{
    Model, Panel,
    panels::{runs, sessions},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};
use std::collections::HashSet;

pub fn render(model: &Model, frame: &mut Frame<'_>, area: Rect, offline: bool) {
    let scope = if model.runs_all_loops {
        "all loops"
    } else {
        &model.settings.preset.loop_name
    };
    let suffix = if model.runs.filter.is_empty() {
        String::new()
    } else {
        format!(" · /{}", model.runs.filter)
    };
    let border = if model.focus == Panel::Runs {
        model.theme.emphasis
    } else {
        model.theme.muted
    };
    let listed = model
        .runs
        .items
        .iter()
        .map(|row| row.id.as_str())
        .collect::<HashSet<_>>();
    let lines = if model.runs.items.is_empty() {
        let empty = if !model.runs.filter.is_empty() {
            "no match".into()
        } else if model.runs_all_loops {
            "no runs".into()
        } else {
            format!(
                "no runs for {} — press * for all",
                model.settings.preset.loop_name
            )
        };
        vec![Line::from(empty)]
    } else {
        model
            .runs
            .items
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let selected = model.runs.selected == Some(index);
                let child = !row.parent_id.is_empty() && listed.contains(row.parent_id.as_str());
                let glyph = runs::glyph(&row.status);
                let status = match glyph {
                    "✓" => model.theme.success,
                    "✗" | "!" => model.theme.error,
                    "?" | "⏸" => model.theme.warning,
                    _ => model.theme.default,
                };
                let short = if row.id.chars().count() > 8 {
                    row.id
                        .chars()
                        .skip(row.id.chars().count() - 8)
                        .collect::<String>()
                } else {
                    row.id.clone()
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
                    Span::raw(if child { "  " } else { "" }),
                    Span::styled(format!("{glyph} "), status),
                    Span::raw(format!("{short} {}  ", row.loop_name)),
                    Span::styled(
                        sessions::relative(row.last_progress_at.as_ref(), model.now_unix),
                        model.theme.muted,
                    ),
                ])
            })
            .collect()
    };
    let mut text = Text::from(lines);
    if let Some(aggregates) = &model.runs_aggregates {
        text.lines.push(Line::styled(
            format!(
                "live {} · done {} · failed {}",
                aggregates.live, aggregates.succeeded, aggregates.failed
            ),
            model.theme.muted,
        ));
    }
    if model.runs_stale {
        text.lines
            .push(Line::styled("runs: stale", model.theme.muted));
    }
    let title = format!("[2] Deliver runs · {scope}{suffix}");
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
