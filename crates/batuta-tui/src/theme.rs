use ratatui::style::{Color, Modifier, Style};

#[derive(Clone, Debug)]
pub struct Theme {
    pub color: bool,
    pub default: Style,
    pub muted: Style,
    pub emphasis: Style,
    pub success: Style,
    pub warning: Style,
    pub error: Style,
    pub selection: Style,
}

impl Theme {
    pub fn detect() -> Self {
        Self::new(std::env::var_os("NO_COLOR").is_none())
    }

    pub fn new(color: bool) -> Self {
        let fg = |color_value| {
            if color {
                Style::default().fg(color_value)
            } else {
                Style::default()
            }
        };
        Self {
            color,
            default: fg(Color::Gray),
            muted: fg(Color::DarkGray).add_modifier(Modifier::DIM),
            emphasis: fg(Color::White).add_modifier(Modifier::BOLD),
            success: fg(Color::Green).add_modifier(Modifier::BOLD),
            warning: fg(Color::Yellow).add_modifier(Modifier::BOLD),
            error: fg(Color::Red).add_modifier(Modifier::BOLD),
            selection: fg(Color::Cyan).add_modifier(Modifier::REVERSED),
        }
    }
}
