use crate::app::{
    AttentionSource, Model, Panel,
    panels::{attention, sessions},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};

pub fn render(model: &Model, frame: &mut Frame<'_>, area: Rect, offline: bool) {
    let hidden = model
        .attention_overview_total
        .saturating_sub(model.attention.len() as u64);
    let title = if hidden > 0 {
        format!("[3] Attention ({}, +{hidden} more)", model.attention.len())
    } else {
        format!("[3] Attention ({})", model.attention.len())
    };
    let border = if model.focus == Panel::Attention {
        model.theme.emphasis
    } else {
        model.theme.muted
    };
    let lines = if model.attention.is_empty() {
        vec![Line::from("nothing needs attention")]
    } else {
        model
            .attention
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let kind = match item.source.as_ref() {
                    Some(AttentionSource::Permission { .. }) => "permission",
                    Some(AttentionSource::Clarification { .. }) => "clarify",
                    Some(AttentionSource::Overview { kind, .. }) => kind,
                    None => "attention",
                };
                let selected = model.attention_selected == Some(index);
                Line::from(vec![
                    Span::styled(
                        if selected { "> " } else { "  " },
                        if selected {
                            model.theme.selection
                        } else {
                            model.theme.default
                        },
                    ),
                    Span::styled(
                        format!("{} ", attention::source_glyph(item)),
                        model.theme.warning,
                    ),
                    Span::raw(format!("{kind}  {}  ", item.title)),
                    Span::styled(
                        sessions::relative(item.at.as_ref(), model.now_unix),
                        model.theme.muted,
                    ),
                ])
            })
            .collect()
    };
    let mut text = Text::from(lines);
    if model.attention_overview_unavailable {
        text.lines
            .push(Line::styled("overview unavailable", model.theme.muted));
    }
    if let Some(prompt) = attention::confirm_prompt(model) {
        text.lines.push(Line::styled(prompt, model.theme.warning));
    }
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
