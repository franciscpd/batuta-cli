use crate::app::Model;
use ratatui::{Frame, layout::Rect};

pub fn render(model: &Model, frame: &mut Frame<'_>, area: Rect) {
    let Some(detail) = model.session_detail() else {
        return;
    };
    detail.composer.render(area, frame);
    let hint = if detail.composer.focused {
        "Alt+Enter newline · Esc back"
    } else {
        "i edit · Enter send"
    };
    let width = hint.chars().count() as u16;
    let hint_area = Rect::new(
        area.right().saturating_sub(width),
        area.bottom().saturating_sub(1),
        width.min(area.width),
        1,
    );
    frame.render_widget(
        ratatui::widgets::Paragraph::new(hint).style(model.theme.muted),
        hint_area,
    );
}
