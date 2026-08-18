use crate::app::{FooterState, Model, RenderCacheKey};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Text},
    widgets::{Paragraph, Wrap},
};
use std::ops::Range;

pub fn visible_entry_range(model: &Model, height: u16) -> Range<usize> {
    let visible = usize::from(height).saturating_add(2);
    let start = if model.follow {
        model.transcript.len().saturating_sub(visible)
    } else {
        model.selection.saturating_sub(visible / 2)
    };
    start..(start + visible).min(model.transcript.len())
}

pub fn render(model: &Model, frame: &mut Frame<'_>, area: Rect) {
    if model.transcript.is_empty() {
        frame.render_widget(
            Paragraph::new("no transcript yet").style(model.theme.muted),
            area,
        );
        return;
    }
    let range = visible_entry_range(model, area.height);
    let start = range.start;
    let mut lines = Vec::new();
    if model.warning.is_some() {
        lines.push(Line::styled(
            format!("warning: {}", model.warning.as_deref().unwrap_or_default()),
            model.theme.warning,
        ));
    }
    if start == 0 && model.beginning {
        lines.push(Line::styled("beginning of transcript", model.theme.muted));
    }
    if start == 0 {
        for marker in &model.transcript.markers {
            lines.push(Line::styled(
                format!("─ resynchronized ({}) ─", marker.reason),
                model.theme.muted,
            ));
        }
    }
    for (index, entry) in model
        .transcript
        .entries()
        .into_iter()
        .enumerate()
        .take(range.end)
        .skip(start)
    {
        let key = RenderCacheKey {
            start_sequence: entry.start_sequence,
            sequence: entry.sequence,
            width: model.size.0.saturating_sub(4),
            reasoning_expanded: model.reasoning_expanded,
            expanded: model.expanded.contains(&entry.start_sequence),
            color: model.theme.color,
        };
        if let Some(text) = model.render_cache.get(&key) {
            for mut line in text.lines.clone() {
                if index == model.selection {
                    line = line.style(model.theme.selection);
                }
                lines.push(line);
            }
        }
    }
    let mut paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
    if matches!(model.footer, FooterState::Offline) {
        paragraph = paragraph.style(model.theme.muted.add_modifier(Modifier::DIM));
    }
    frame.render_widget(paragraph, area);
}
