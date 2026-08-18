use crate::app::model::RunDetail;
use compozy_client::types::{LoopEvent, LoopGenerationOutput};

#[derive(Clone, Copy)]
pub enum Target<'a> {
    Child(&'a str),
    Session(&'a str),
    Gate { id: &'a str, revision: i64 },
}

pub fn targets(detail: &RunDetail) -> Vec<Target<'_>> {
    let mut targets = Vec::new();
    if let Some(run) = &detail.run {
        for generation in &run.generations {
            for output in &generation.outputs {
                if !output.child_loop_run_id.is_empty() {
                    targets.push(Target::Child(&output.child_loop_run_id));
                }
                if !output.session_id.is_empty() {
                    targets.push(Target::Session(&output.session_id));
                }
            }
        }
    }
    targets.extend(detail.events.iter().filter_map(gate));
    targets
}

pub fn gate(event: &LoopEvent) -> Option<Target<'_>> {
    (event.kind == "needs_approval").then(|| Target::Gate {
        id: event
            .payload
            .get("gate_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("—"),
        revision: event
            .payload
            .get("revision")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or_default(),
    })
}

pub fn selected_target(detail: &RunDetail) -> Option<Target<'_>> {
    targets(detail).get(detail.selection).copied()
}

pub fn terminal(status: &str) -> bool {
    crate::app::panels::runs::terminal(status)
}

pub fn runtime(output: &LoopGenerationOutput) -> String {
    let Some(value) = &output.resolved_runtime else {
        return String::new();
    };
    let provider = value
        .get("provider")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let model = value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    match (provider.is_empty(), model.is_empty()) {
        (false, false) => format!("{provider}/{model}"),
        (false, true) => provider.to_owned(),
        (true, false) => model.to_owned(),
        (true, true) => String::new(),
    }
}

pub fn event_detail(event: &LoopEvent) -> String {
    let text = |key: &str| {
        event
            .payload
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
    };
    match event.kind.as_str() {
        "node_running" | "node_succeeded" | "node_failed" | "node_retry_scheduled" => {
            text("node_id").to_owned()
        }
        "status_changed" => text("status").to_owned(),
        "generation_started" => format!(
            "gen {}",
            event
                .payload
                .get("generation")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or_default()
        ),
        "needs_approval" => format!(
            "gate {} (rev {})  a approve · x reject",
            text("gate_id"),
            event
                .payload
                .get("revision")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or_default()
        ),
        kind if kind.starts_with("node_") => text("node_id").to_owned(),
        _ => String::new(),
    }
}

pub fn time(raw: &str) -> &str {
    raw.get(11..19).unwrap_or(raw)
}
