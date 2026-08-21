use ratatui::style::{Color, Modifier, Style};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ThemeVariant {
    #[default]
    Auto,
    Dark,
    Light,
}

impl ThemeVariant {
    pub fn resolve(self, colorfgbg: Option<&str>) -> Self {
        match self {
            Self::Auto => match colorfgbg
                .and_then(|value| value.rsplit(';').next())
                .and_then(|value| value.parse::<u8>().ok())
            {
                Some(7 | 15) => Self::Light,
                _ => Self::Dark,
            },
            variant => variant,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticToken {
    Text,
    Muted,
    Active,
    Success,
    Waiting,
    Error,
    Destructive,
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityState {
    Thinking,
    Tool,
    Waiting,
    Completed,
    Failed,
}

impl ActivityState {
    pub const fn marker(self) -> &'static str {
        match self {
            Self::Thinking => "…",
            Self::Tool => "▶",
            Self::Waiting => "!",
            Self::Completed => "✓",
            Self::Failed => "×",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Thinking => "thinking",
            Self::Tool => "tool",
            Self::Waiting => "waiting",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub const fn token(self) -> SemanticToken {
        match self {
            Self::Completed => SemanticToken::Success,
            Self::Failed => SemanticToken::Error,
            Self::Thinking | Self::Tool | Self::Waiting => SemanticToken::Waiting,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub color: bool,
    pub variant: ThemeVariant,
    /// Normal prose deliberately inherits the terminal's foreground/background.
    pub default: Style,
    pub muted: Style,
    pub emphasis: Style,
    pub success: Style,
    pub warning: Style,
    pub error: Style,
    pub destructive: Style,
    pub system: Style,
    pub selection: Style,
}

impl Theme {
    pub fn detect() -> Self {
        Self::with_variant(
            std::env::var_os("NO_COLOR").is_none(),
            ThemeVariant::Auto,
            std::env::var("COLORFGBG").ok().as_deref(),
        )
    }

    pub fn new(color: bool) -> Self {
        Self::with_variant(color, ThemeVariant::Dark, None)
    }

    pub fn with_variant(color: bool, variant: ThemeVariant, colorfgbg: Option<&str>) -> Self {
        let variant = variant.resolve(colorfgbg);
        let colored = |color_value| {
            if color {
                Style::default().fg(color_value)
            } else {
                Style::default()
            }
        };
        let (muted, active, success, waiting, error, system) = match variant {
            ThemeVariant::Dark | ThemeVariant::Auto => (
                Color::DarkGray,
                Color::Cyan,
                Color::Green,
                Color::Yellow,
                Color::Red,
                Color::Magenta,
            ),
            ThemeVariant::Light => (
                Color::Gray,
                Color::Blue,
                Color::Green,
                Color::Yellow,
                Color::Red,
                Color::Magenta,
            ),
        };
        Self {
            color,
            variant,
            default: Style::default(),
            muted: colored(muted).add_modifier(Modifier::DIM),
            emphasis: colored(active).add_modifier(Modifier::BOLD),
            success: colored(success).add_modifier(Modifier::BOLD),
            warning: colored(waiting).add_modifier(Modifier::BOLD),
            error: colored(error).add_modifier(Modifier::BOLD),
            destructive: colored(error).add_modifier(Modifier::BOLD),
            system: colored(system).add_modifier(Modifier::DIM),
            selection: colored(active).add_modifier(Modifier::REVERSED),
        }
    }

    pub const fn style(&self, token: SemanticToken) -> Style {
        match token {
            SemanticToken::Text => self.default,
            SemanticToken::Muted => self.muted,
            SemanticToken::Active => self.emphasis,
            SemanticToken::Success => self.success,
            SemanticToken::Waiting => self.warning,
            SemanticToken::Error => self.error,
            SemanticToken::Destructive => self.destructive,
            SemanticToken::System => self.system,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ut_700_default_prose_inherits_terminal_colours() {
        for variant in [ThemeVariant::Dark, ThemeVariant::Light] {
            let theme = Theme::with_variant(true, variant, None);
            assert_eq!(theme.default.fg, None);
            assert_eq!(theme.default.bg, None);
        }
    }

    #[test]
    fn ut_701_semantic_tokens_use_only_ansi_16_colours() {
        for variant in [ThemeVariant::Dark, ThemeVariant::Light] {
            let theme = Theme::with_variant(true, variant, None);
            assert!(matches!(
                theme.style(SemanticToken::Active).fg,
                Some(Color::Cyan | Color::Blue)
            ));
            assert!(matches!(
                theme.style(SemanticToken::Success).fg,
                Some(Color::Green)
            ));
            assert!(matches!(
                theme.style(SemanticToken::Waiting).fg,
                Some(Color::Yellow)
            ));
            assert!(matches!(
                theme.style(SemanticToken::Error).fg,
                Some(Color::Red)
            ));
            assert!(matches!(
                theme.style(SemanticToken::Destructive).fg,
                Some(Color::Red)
            ));
            assert!(matches!(
                theme.style(SemanticToken::System).fg,
                Some(Color::Magenta)
            ));
        }
    }

    #[test]
    fn ut_702_states_have_non_colour_markers() {
        for state in [
            ActivityState::Thinking,
            ActivityState::Tool,
            ActivityState::Waiting,
            ActivityState::Completed,
            ActivityState::Failed,
        ] {
            assert!(!state.marker().is_empty());
            assert!(!state.label().is_empty());
        }
    }

    #[test]
    fn ut_704_no_color_removes_colour_attributes() {
        let theme = Theme::with_variant(false, ThemeVariant::Light, None);
        for token in [
            SemanticToken::Text,
            SemanticToken::Muted,
            SemanticToken::Active,
            SemanticToken::Success,
            SemanticToken::Waiting,
            SemanticToken::Error,
            SemanticToken::Destructive,
            SemanticToken::System,
        ] {
            let style = theme.style(token);
            assert_eq!(style.fg, None);
            assert_eq!(style.bg, None);
        }
    }

    #[test]
    fn ut_705_auto_variant_resolves_colorfgbg_deterministically() {
        for input in [
            Some("0"),
            Some("15;8"),
            Some("7"),
            Some("0;15"),
            None,
            Some("bad"),
            Some("0;42"),
        ] {
            let expected = match input {
                Some("7" | "0;15") => ThemeVariant::Light,
                _ => ThemeVariant::Dark,
            };
            assert_eq!(ThemeVariant::Auto.resolve(input), expected);
        }
    }

    #[test]
    fn ut_719_activity_states_have_one_label_marker_and_token() {
        assert_eq!(ActivityState::Thinking.token(), SemanticToken::Waiting);
        assert_eq!(ActivityState::Tool.token(), SemanticToken::Waiting);
        assert_eq!(ActivityState::Waiting.token(), SemanticToken::Waiting);
        assert_eq!(ActivityState::Completed.token(), SemanticToken::Success);
        assert_eq!(ActivityState::Failed.token(), SemanticToken::Error);
    }
}
