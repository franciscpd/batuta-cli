use crate::app::{AppMode, FooterState, Model};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    widgets::Paragraph,
};

pub fn render(model: &Model, frame: &mut Frame<'_>, area: Rect) {
    let confirm = model
        .session_detail()
        .and_then(|detail| detail.confirm.as_ref())
        .map(|confirm| confirm.prompt.as_str());
    let composer_rows = u16::from(model.mode == AppMode::Full) * 3;
    let confirm_rows = u16::from(confirm.is_some());
    let regions = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(confirm_rows),
        Constraint::Length(1),
        Constraint::Length(composer_rows),
    ])
    .split(area);
    super::transcript::render(model, frame, regions[0]);
    if let Some(confirm) = confirm {
        frame.render_widget(
            Paragraph::new(confirm).style(model.theme.warning),
            regions[1],
        );
    }
    frame.render_widget(
        Paragraph::new(footer(model)).style(model.theme.muted),
        regions[2],
    );
    if composer_rows > 0 {
        super::composer::render(model, frame, regions[3]);
    }
    super::chooser::render(model, frame, area);
}

fn footer(model: &Model) -> String {
    let Some(detail) = model.session_detail() else {
        return String::new();
    };
    match &detail.view.footer {
        FooterState::Live => "live".into(),
        FooterState::Stopped { .. } => "stopped — sending a prompt restarts it".into(),
        FooterState::Reconnecting(attempt) => {
            format!("stream lost — reconnecting (attempt {attempt})")
        }
        FooterState::Offline => "daemon offline — last state shown".into(),
        FooterState::Resynchronized(reason) => format!("resynchronized ({reason})"),
        FooterState::NewBelow(count) => format!("{count} new below — G to jump"),
        FooterState::Fatal(error) => format!("stream stopped — {error}"),
    }
}
