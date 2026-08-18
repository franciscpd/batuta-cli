use crate::app::Model;
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Clear, Paragraph},
};
pub fn render(model: &Model, frame: &mut Frame<'_>) {
    let Some(toast) = &model.toast else { return };
    let width = (toast.text.chars().count() as u16 + 2).min(frame.area().width);
    let area = Rect {
        x: frame.area().right().saturating_sub(width),
        y: frame.area().bottom().saturating_sub(2),
        width,
        height: 1,
    };
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(toast.text.as_str()).style(match toast.kind {
            crate::app::ToastKind::Success => model.theme.success,
            crate::app::ToastKind::Error => model.theme.error,
            crate::app::ToastKind::Info => model.theme.emphasis,
        }),
        area,
    );
}
