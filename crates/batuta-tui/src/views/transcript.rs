use crate::{
    app::{FooterState, Model, RenderCacheKey},
    transcript::PresentationRow,
};
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
    let rows = detail.transcript.presentation_rows(detail.view.raw_debug);
    let visible = usize::from(height).saturating_add(2);
    let start = if detail.view.follow {
        rows.len().saturating_sub(visible)
    } else {
        detail.view.selection.saturating_sub(visible / 2)
    };
    start..(start + visible).min(rows.len())
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
    if detail.view.raw_debug {
        lines.push(Line::styled(
            "DEBUG · raw transcript presentation",
            model.theme.warning,
        ));
    }
    let entries = detail.transcript.entries();
    macro_rules! render_cached_entry {
        ($entry:expr, $selected:expr) => {{
            let entry = $entry;
            let key = RenderCacheKey::for_entry(
                entry,
                model.size.0.saturating_sub(4),
                detail.view.reasoning_expanded,
                detail.view.raw_debug,
                detail.view.expanded.contains(&entry.start_sequence),
                model.theme.color,
                model.theme.variant,
            );
            if let Some(text) = detail.view.render_cache.get(&key) {
                for mut line in text.lines.clone() {
                    if $selected {
                        line = line.style(model.theme.selection);
                    }
                    lines.push(line);
                }
            }
        }};
    }
    for (index, row) in detail
        .transcript
        .presentation_rows(detail.view.raw_debug)
        .into_iter()
        .enumerate()
        .take(range.end)
        .skip(start)
    {
        let selected = index == detail.view.selection;
        match row {
            PresentationRow::Entry { entry_index } => {
                render_cached_entry!(entries[entry_index], selected);
            }
            PresentationRow::Group {
                entry_indexes,
                label,
            } => {
                let first = entries[entry_indexes[0]];
                if detail.view.expanded.contains(&first.start_sequence) {
                    for entry_index in entry_indexes {
                        render_cached_entry!(entries[entry_index], selected);
                    }
                } else {
                    let line = Line::styled(
                        format!("▶ {} {}  Enter expand", entry_indexes.len(), label),
                        model.theme.muted,
                    );
                    lines.push(if selected {
                        line.style(model.theme.selection)
                    } else {
                        line
                    });
                }
            }
        }
    }
    let mut paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
    if matches!(detail.view.footer, FooterState::Offline) {
        paragraph = paragraph.style(model.theme.muted.add_modifier(Modifier::DIM));
    }
    frame.render_widget(paragraph, area);
}
