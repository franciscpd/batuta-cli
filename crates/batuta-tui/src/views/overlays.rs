use crate::{
    app::{Model, Overlay},
    keymap,
};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Clear, Paragraph},
};
fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(4));
    let height = height.min(area.height.saturating_sub(2));
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}
pub fn render(model: &Model, frame: &mut Frame<'_>) {
    let Some(overlay) = &model.overlay else {
        return;
    };
    let (title, text, scroll) = match overlay {
        Overlay::Help { scroll } => ("help", keymap::help_lines().join("\n"), *scroll),
        Overlay::Logs { .. } => ("logs", "no logs".to_owned(), 0),
        Overlay::WorkspacePicker { items, .. } => (
            "workspaces",
            items
                .iter()
                .map(|item| format!("{}  {}", item.name, item.root_dir))
                .collect::<Vec<_>>()
                .join("\n"),
            0,
        ),
        Overlay::Clarify {
            question,
            choices,
            text,
            ..
        } => (
            "clarify",
            if choices.is_empty() {
                format!("{question}\nanswer: {text}")
            } else {
                format!(
                    "{question}\n{}",
                    choices
                        .iter()
                        .enumerate()
                        .map(|(index, value)| format!("{}  {value}", index + 1))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            },
            0,
        ),
    };
    let area = centered(frame.area(), 70, frame.area().height.saturating_sub(4));
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(text)
            .scroll((scroll as u16, 0))
            .block(Block::default().title(title).borders(Borders::ALL))
            .style(model.theme.default),
        area,
    );
}
