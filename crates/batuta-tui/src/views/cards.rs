use crate::theme::Theme;
use ratatui::style::Style;

pub fn tool_status(state: Option<&str>, theme: &Theme) -> (&'static str, Style) {
    match state {
        Some("input-streaming") => ("⋯", theme.muted),
        Some("input-available") => ("▶", theme.emphasis),
        Some("output-error") => ("✗", theme.error),
        _ => ("✓", theme.success),
    }
}
