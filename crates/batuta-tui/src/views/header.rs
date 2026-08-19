use crate::app::{DaemonState, Model};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};

pub fn offline(model: &Model) -> bool {
    model.is_offline()
}
pub fn has_banner(model: &Model) -> bool {
    let Some(version) = model.daemon.version.as_deref() else {
        return true;
    };
    if version.is_empty() || version == "dev" {
        return true;
    }
    let normalized = version.strip_prefix('v').unwrap_or(version);
    let normalized = normalized.split("-g").next().unwrap_or(normalized);
    let floor = semver::Version::parse("0.3.0-beta.16").expect("static version floor");
    semver::Version::parse(normalized).map_or(true, |version| version < floor)
}
pub fn text(model: &Model, width: u16) -> String {
    let workspace = model
        .workspace
        .as_ref()
        .map(|value| value.name.as_str())
        .unwrap_or("none");
    let state = model.daemon_state();
    let status = match state {
        DaemonState::Offline => "offline",
        DaemonState::Draining => "draining — finishing in-flight work, writes refused",
        DaemonState::Connected => "running",
    };
    let version = if state == DaemonState::Offline {
        String::new()
    } else {
        model
            .daemon
            .version
            .as_ref()
            .map(|value| format!(" {value}"))
            .unwrap_or_default()
    };
    let suffix = format!(
        " · daemon {status}{version} · {} attention",
        model.attention.len()
    );
    let prefix = "batuta · ws: ";
    let marker = " ▾";
    let available = usize::from(width)
        .saturating_sub(prefix.chars().count() + marker.chars().count() + suffix.chars().count());
    let shown = if workspace.chars().count() > available {
        if available > 1 {
            format!(
                "{}…",
                workspace.chars().take(available - 1).collect::<String>()
            )
        } else {
            "…".into()
        }
    } else {
        workspace.to_owned()
    };
    format!("{prefix}{shown}{marker}{suffix}")
}
pub fn render(model: &Model, frame: &mut Frame<'_>, area: Rect) {
    let line = text(model, area.width);
    let style = match model.daemon_state() {
        DaemonState::Draining => model.theme.warning,
        DaemonState::Offline => model.theme.error,
        DaemonState::Connected => model.theme.emphasis,
    };
    frame.render_widget(
        Paragraph::new(line).style(style),
        Rect { height: 1, ..area },
    );
    if area.height > 1 {
        frame.render_widget(
            Paragraph::new(
                "warning: daemon version dev — compatibility unverified (floor v0.3.0-beta.16)",
            )
            .style(model.theme.warning),
            Rect {
                y: area.y + 1,
                height: 1,
                ..area
            },
        );
    }
}
