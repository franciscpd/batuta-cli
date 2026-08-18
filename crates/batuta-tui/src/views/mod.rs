pub mod cards;
pub mod footer;
pub mod header;
pub mod markdown;
pub mod transcript;

use crate::app::Model;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    text::Line,
    widgets::Paragraph,
};

pub fn view(model: &Model, frame: &mut Frame<'_>) {
    let area = frame.area();
    if area.width < 20 {
        frame.render_widget(
            Paragraph::new("terminal too narrow").style(model.theme.warning),
            area,
        );
        return;
    }
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);
    header::render(model, frame, rows[0]);
    frame.render_widget(
        Paragraph::new(Line::from("─".repeat(usize::from(area.width)))).style(model.theme.muted),
        rows[1],
    );
    transcript::render(model, frame, rows[2]);
    frame.render_widget(
        Paragraph::new(Line::from("─".repeat(usize::from(area.width)))).style(model.theme.muted),
        rows[3],
    );
    footer::render(model, frame, rows[4]);
}
