use crate::app::{FooterState, Model};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::Paragraph,
};

const KEYS: &str = "j/k move  Enter expand  t reasoning  G newest  g oldest  q quit";
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
    header(model, frame, rows[0]);
    frame.render_widget(
        Paragraph::new(Line::from("─".repeat(usize::from(area.width)))).style(model.theme.muted),
        rows[1],
    );
    super::transcript::render(model, frame, rows[2]);
    frame.render_widget(
        Paragraph::new(Line::from("─".repeat(usize::from(area.width)))).style(model.theme.muted),
        rows[3],
    );
    footer(model, frame, rows[4]);
}
fn header(model: &Model, frame: &mut Frame<'_>, area: Rect) {
    let Some(detail) = model.session_detail() else {
        return;
    };
    let name = detail.session.name.as_deref().unwrap_or("—");
    let workspace = model
        .workspace
        .as_ref()
        .map(|value| value.name.as_str())
        .unwrap_or("none");
    let prefix = format!(
        "batuta · {workspace} · {} · {} · \"",
        detail.session.id, detail.session.agent_name
    );
    let suffix = format!("\" · {}", detail.session.state);
    let available =
        usize::from(area.width).saturating_sub(prefix.chars().count() + suffix.chars().count());
    let shown = if name.chars().count() <= available {
        name.to_owned()
    } else if available > 1 {
        format!("{}…", name.chars().take(available - 1).collect::<String>())
    } else {
        "…".into()
    };
    frame.render_widget(
        Paragraph::new(format!("{prefix}{shown}{suffix}")).style(model.theme.emphasis),
        area,
    );
}
fn status(model: &Model) -> String {
    let Some(detail) = model.session_detail() else {
        return String::new();
    };
    match &detail.view.footer {
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
fn footer(model: &Model, frame: &mut Frame<'_>, area: Rect) {
    let status = status(model);
    let width = status.chars().count().min(usize::from(area.width)) as u16;
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(2),
            Constraint::Length(width),
        ])
        .split(area);
    frame.render_widget(Paragraph::new(KEYS).style(model.theme.muted), columns[0]);
    frame.render_widget(
        Paragraph::new(status)
            .right_aligned()
            .style(model.theme.emphasis),
        columns[2],
    );
}
