use crate::{
    app::model::{Model, RunRow},
    cmd::{Cmd, Request},
};
use compozy_client::types::LoopRunPage;

pub fn request(model: &mut Model) -> Option<Cmd> {
    let workspace = model.workspace.as_ref()?.id.clone();
    let loop_name = (!model.runs_all_loops).then(|| model.settings.preset.loop_name.clone());
    let limit = model.settings.ui.runs_limit;
    Some(Cmd::Get(model.allocate(|id| Request::Runs {
        id,
        workspace,
        loop_name,
        limit,
    })))
}

pub fn apply(model: &mut Model, page: LoopRunPage) {
    let selected_id = model.runs.selected().map(|row| row.id.clone());
    model.runs_aggregates = Some(page.aggregates);
    model.runs_unfiltered = page
        .runs
        .into_iter()
        .map(|run| RunRow {
            id: run.id,
            loop_name: run.loop_name,
            status: run.status,
            parent_id: run.parent_loop_run_id,
            last_progress_at: Some(run.last_progress_at),
        })
        .collect();
    refilter(model, selected_id.as_deref());
    model.runs_stale = false;
}

pub fn refilter(model: &mut Model, selected_id: Option<&str>) {
    let needle = model.runs.filter.to_lowercase();
    let items = model
        .runs_unfiltered
        .iter()
        .filter(|row| {
            needle.is_empty()
                || row.id.to_lowercase().contains(&needle)
                || row.loop_name.to_lowercase().contains(&needle)
                || row.status.to_lowercase().contains(&needle)
        })
        .cloned()
        .collect::<Vec<_>>();
    model.runs.set_items(items);
    if let Some(id) = selected_id
        && let Some(index) = model.runs.items.iter().position(|row| row.id == id)
    {
        model.runs.selected = Some(index);
    }
}

pub fn glyph(status: &str) -> &'static str {
    match status {
        "running" => "●",
        "done" | "succeeded" | "completed" => "✓",
        "failed" => "✗",
        "canceled" | "cancelled" | "killed" => "■",
        "no-op" | "no_op" | "noop" => "◌",
        "paused" => "⏸",
        "needs_approval" => "?",
        "blocked" | "exhausted" | "stalled" | "needs_attention" => "!",
        _ => "?",
    }
}

pub fn terminal(status: &str) -> bool {
    matches!(
        status,
        "done"
            | "succeeded"
            | "completed"
            | "failed"
            | "canceled"
            | "cancelled"
            | "killed"
            | "no-op"
            | "no_op"
            | "noop"
    )
}

pub fn any_live(model: &Model) -> bool {
    model.runs.items.iter().any(|row| !terminal(&row.status))
}
