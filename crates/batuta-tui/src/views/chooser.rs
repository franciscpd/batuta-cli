use crate::app::Model;
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph},
};

pub fn render(model: &Model, frame: &mut Frame<'_>, area: Rect) {
    let Some(detail) = model.session_detail() else {
        return;
    };
    let Some(chooser) = &detail.chooser else {
        return;
    };
    let turn = detail
        .session
        .activity
        .as_ref()
        .and_then(|activity| activity.turn_id.as_deref())
        .unwrap_or("unknown");
    let width = area.width.saturating_sub(4).min(62);
    let height = 7.min(area.height);
    let popup = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    let options = [
        ("queue", "send after the current turn"),
        ("steer", "inject into the current turn"),
        ("interrupt", "stop the current turn, then send"),
    ];
    let lines = options
        .into_iter()
        .enumerate()
        .map(|(index, (name, help))| {
            let marker = if index == chooser.selected { ">" } else { " " };
            let style = if index == chooser.selected {
                model.theme.emphasis.add_modifier(Modifier::BOLD)
            } else {
                model.theme.default
            };
            Line::styled(format!("{marker} {name:<10} {help}"), style)
        })
        .collect::<Vec<_>>();
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(format!("session is busy (turn {turn})"))
                .borders(Borders::ALL),
        ),
        popup,
    );
}
