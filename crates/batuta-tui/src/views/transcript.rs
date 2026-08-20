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
    let Some(detail) = model.session_detail() else {
        return 0..0;
    };
    let visible = usize::from(height).saturating_add(2);
    let start = if detail.view.follow {
        detail.transcript.len().saturating_sub(visible)
    } else {
        detail.view.selection.saturating_sub(visible / 2)
    };
    start..(start + visible).min(detail.transcript.len())
}
pub fn render(model: &Model, frame: &mut Frame<'_>, area: Rect) {
    let Some(detail) = model.session_detail() else {
        return;
    };
    if detail.transcript.is_empty()
        && detail.transcript.markers.is_empty()
        && detail.view.warning.is_none()
    {
        frame.render_widget(
            Paragraph::new("no transcript yet").style(model.theme.muted),
            area,
        );
        return;
    }
    let range = visible_entry_range(model, area.height);
    let start = range.start;
    let mut lines = Vec::new();
    if let Some(warning) = &detail.view.warning {
        lines.push(Line::styled(
            format!("warning: {warning}"),
            model.theme.warning,
        ));
    }
    if start == 0 && detail.view.beginning {
        lines.push(Line::styled("beginning of transcript", model.theme.muted));
    }
    if start == 0 {
        for marker in &detail.transcript.markers {
            lines.push(Line::styled(
                format!("─ resynchronized ({}) ─", marker.reason),
                model.theme.muted,
            ));
        }
    }
    for (index, entry) in detail
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
            reasoning_expanded: detail.view.reasoning_expanded,
            expanded: detail.view.expanded.contains(&entry.start_sequence),
            color: model.theme.color,
            theme: model.theme.variant,
        };
        if let Some(text) = detail.view.render_cache.get(&key) {
            for mut line in text.lines.clone() {
                if index == detail.view.selection {
                    line = line.style(model.theme.selection);
                }
                lines.push(line);
            }
        }
    }
    let mut paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
    if matches!(detail.view.footer, FooterState::Offline) {
        paragraph = paragraph.style(model.theme.muted.add_modifier(Modifier::DIM));
    }
    frame.render_widget(paragraph, area);
}
