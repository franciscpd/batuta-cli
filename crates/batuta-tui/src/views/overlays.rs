use crate::{
    app::{Model, Overlay},
    keymap,
};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
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
    if let Some((title, lines, selected)) = super::logs::content(model) {
        let area = centered(
            frame.area(),
            frame.area().width.saturating_sub(4),
            frame.area().height.saturating_sub(2),
        );
        let height = usize::from(area.height.saturating_sub(2)).max(1);
        let scroll = selected.saturating_sub(height.saturating_sub(1)) as u16;
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines)
                .scroll((scroll, 0))
                .block(Block::default().title(title).borders(Borders::ALL))
                .style(model.theme.default),
            area,
        );
        return;
    }
    if let Some((title, text)) = super::picker::content(model) {
        let area = centered(frame.area(), 70, frame.area().height.saturating_sub(4));
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(text)
                .block(Block::default().title(title).borders(Borders::ALL))
                .style(model.theme.default),
            area,
        );
        return;
    }
    let (title, text, scroll) = match overlay {
        Overlay::Help { scroll } => ("help".to_owned(), keymap::help_lines().join("\n"), *scroll),
        Overlay::WorkspaceOnboarding {
            candidate,
            confirming,
            adding,
            booting,
            message,
            technical_details,
            details_expanded,
            ..
        } => {
            let (title, action) = if *booting {
                ("Workspace added", "✓ workspace added\n… starting workspace")
            } else if *adding {
                ("Add workspace?", "… adding workspace")
            } else if *confirming {
                (
                    "Add workspace?",
                    "This registers the directory with the connected daemon.\nEnter confirm                                      Esc cancel",
                )
            } else if message.is_some() {
                (
                    "Workspace not registered",
                    "[r] refresh\n[w] choose an existing workspace\n[q] exit",
                )
            } else {
                (
                    "Workspace not registered",
                    "[a] add this directory\n[w] choose an existing workspace\n[q] exit",
                )
            };
            let message = message.as_deref().unwrap_or("");
            let details = technical_details
                .as_deref()
                .map_or(String::new(), |details| {
                    if *details_expanded {
                        format!("\n\nTechnical details\n{details}\n[d] hide details")
                    } else {
                        "\n\n[d] show technical details".into()
                    }
                });
            (
                title.into(),
                format!(
                    "Name   {}\nPath   {}\n\n{}\n{}{}",
                    candidate.name, candidate.root_dir, action, message, details
                ),
                0,
            )
        }
        Overlay::Logs { .. } | Overlay::WorkspacePicker { .. } => unreachable!(),
        Overlay::Clarify { .. } => {
            let (title, text) = super::clarify::content(overlay).expect("clarification overlay");
            (title, text, 0)
        }
    };
    let area = centered(frame.area(), 70, frame.area().height.saturating_sub(4));
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(text)
            .scroll((scroll as u16, 0))
            .wrap(Wrap { trim: false })
            .block(Block::default().title(title).borders(Borders::ALL))
            .style(model.theme.default),
        area,
    );
}
