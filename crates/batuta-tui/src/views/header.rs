use crate::app::Model;
use ratatui::{Frame, layout::Rect, widgets::Paragraph};

pub fn render(model: &Model, frame: &mut Frame<'_>, area: Rect) {
    let name = model.header.name.as_deref().unwrap_or("—");
    let prefix = format!(
        "batuta · {} · {} · {} · \"",
        model.header.workspace, model.header.session_id, model.header.agent
    );
    let suffix = format!("\" · {}", model.header.state);
    let available =
        usize::from(area.width).saturating_sub(prefix.chars().count() + suffix.chars().count());
    let shown_name = if name.chars().count() <= available {
        name.to_owned()
    } else if available > 1 {
        format!("{}…", name.chars().take(available - 1).collect::<String>())
    } else {
        "…".into()
    };
    let line = format!("{prefix}{shown_name}{suffix}");
    frame.render_widget(Paragraph::new(line).style(model.theme.emphasis), area);
}
