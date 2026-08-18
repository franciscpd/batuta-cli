use crate::{
    app::panels::{detail_run, runs},
    app::{Detail, Model},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

pub fn render(model: &Model, frame: &mut Frame<'_>, area: Rect) {
    let Detail::Run(detail) = &model.detail else {
        return;
    };
    let mut lines = Vec::new();
    let Some(value) = &detail.run else {
        frame.render_widget(
            Paragraph::new("loading run…").style(model.theme.muted),
            area,
        );
        return;
    };
    let run = &value.run;
    lines.push(
        Line::from(format!(
            "{} · {} · {} · gen {}",
            run.id, run.loop_name, run.status, run.generation
        ))
        .style(model.theme.emphasis),
    );
    lines.push(
        Line::from(format!(
            "started {}  last progress {}  parent {}",
            detail_run::time(run.started_at.raw()),
            detail_run::time(run.last_progress_at.raw()),
            if run.parent_loop_run_id.is_empty() {
                "—"
            } else {
                &run.parent_loop_run_id
            }
        ))
        .style(model.theme.muted),
    );
    lines.push(Line::from(format!("inputs  {}", inputs(&run.inputs))));
    let newest = value.generations.iter().map(|item| item.generation).max();
    for generation in &value.generations {
        lines.push(
            Line::from(format!("generation {}", generation.generation)).style(model.theme.emphasis),
        );
        if newest == Some(generation.generation) || value.generations.len() == 1 {
            for output in &generation.outputs {
                let fields = [
                    runs::glyph(&output.status).to_owned(),
                    output.node_id.clone(),
                    format!("item {}", output.item_index),
                    format!("attempt {}", output.attempt),
                ];
                lines.push(Line::from(format!("  {}", fields.join("  "))));
                let mut links = Vec::new();
                if !output.child_loop_run_id.is_empty() {
                    links.push(format!("→ {}", output.child_loop_run_id));
                }
                if !output.session_id.is_empty() {
                    links.push(format!("session {}", output.session_id));
                }
                if !links.is_empty() {
                    lines.push(
                        Line::from(format!("      {}", links.join("  "))).style(model.theme.muted),
                    );
                }
                let runtime = detail_run::runtime(output);
                let mut metadata = Vec::new();
                if !runtime.is_empty() {
                    metadata.push(runtime);
                }
                if !output.failure_class.is_empty() {
                    metadata.push(format!("failure {}", output.failure_class));
                }
                if !metadata.is_empty() {
                    lines.push(
                        Line::from(format!("      {}", metadata.join("  ")))
                            .style(model.theme.muted),
                    );
                }
            }
        }
    }
    lines.push(Line::from("events").style(model.theme.emphasis));
    for event in &detail.events {
        let detail_text = detail_run::event_detail(event);
        let text = if event.kind == "needs_approval" {
            format!("  ? {detail_text}")
        } else if detail_text.is_empty() {
            format!("  {} {}", detail_run::time(event.at.raw()), event.kind)
        } else {
            format!(
                "  {} {}  {}",
                detail_run::time(event.at.raw()),
                event.kind,
                detail_text
            )
        };
        let known = matches!(
            event.kind.as_str(),
            "node_running"
                | "node_succeeded"
                | "node_failed"
                | "node_retry_scheduled"
                | "needs_approval"
                | "status_changed"
                | "generation_started"
        ) || event.kind.starts_with("node_");
        let style = if event.kind == "node_failed" {
            model.theme.error
        } else if known {
            model.theme.default
        } else {
            model.theme.muted
        };
        lines.push(Line::from(Span::styled(text, style)));
    }
    lines.push(
        Line::from("p pause  u resume  k kill  Enter open  a/x gate  L logs")
            .style(model.theme.muted),
    );
    frame.render_widget(Paragraph::new(lines), area);
}

fn inputs(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(items) => items
            .iter()
            .map(|(key, value)| format!("{key}={}", scalar(value)))
            .collect::<Vec<_>>()
            .join("  "),
        value => scalar(value),
    }
}

fn scalar(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}
