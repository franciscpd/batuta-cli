use crate::app::{Model, StreamStatus};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};

pub fn offline(model: &Model) -> bool {
    !model.daemon.poll_ok
        && !model.active_streams.is_empty()
        && model.active_streams.iter().all(|id| {
            matches!(
                model.stream_status.get(id),
                Some(StreamStatus::Stale | StreamStatus::Fatal(_))
            )
        })
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
    let status = if offline(model) {
        "offline"
    } else if model.daemon.status == "draining" {
        "draining"
    } else {
        "running"
    };
    let version = if status == "offline" {
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
    frame.render_widget(
        Paragraph::new(line).style(model.theme.emphasis),
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
