use crate::{
    app::{FooterState, Model},
    keymap,
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::Paragraph,
};

pub fn status(model: &Model) -> String {
    match &model.footer {
        FooterState::Live => "live".into(),
        FooterState::Stopped { reason, detail } => {
            let mut value = "stopped — sending a prompt restarts it".to_owned();
            if let Some(reason) = reason {
                value.push_str(&format!(" · {reason}"));
            }
            if let Some(detail) = detail {
                value.push_str(&format!(" · {detail}"));
            }
            value
        }
        FooterState::Reconnecting(attempt) => {
            format!("stream lost — reconnecting (attempt {attempt})")
        }
        FooterState::Offline => "daemon offline — last state shown".into(),
        FooterState::Resynchronized(reason) => format!("resynchronized ({reason})"),
        FooterState::NewBelow(count) => format!("{count} new below — G to jump"),
        FooterState::Fatal(error) => format!("stream stopped — {error}"),
    }
}

pub fn render(model: &Model, frame: &mut Frame<'_>, area: Rect) {
    let status = status(model);
    let status_width = status.chars().count().min(usize::from(area.width)) as u16;
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(2),
            Constraint::Length(status_width),
        ])
        .split(area);
    frame.render_widget(
        Paragraph::new(keymap::footer()).style(model.theme.muted),
        columns[0],
    );
    frame.render_widget(
        Paragraph::new(status)
            .right_aligned()
            .style(model.theme.emphasis),
        columns[2],
    );
}
