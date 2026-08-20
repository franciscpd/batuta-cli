use crate::theme::{ActivityState, Theme};
use ratatui::style::Style;

pub fn tool_status(state: Option<&str>, theme: &Theme) -> (ActivityState, Style) {
    let activity = match state {
        Some("input-streaming") => ActivityState::Thinking,
        Some("input-available") => ActivityState::Tool,
        Some("output-error") => ActivityState::Failed,
        _ => ActivityState::Completed,
    };
    (activity, theme.style(activity.token()))
}
