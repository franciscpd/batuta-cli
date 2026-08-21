use crate::{
    app::{Model, Overlay, WorkspaceRef},
    cmd::{Cmd, Request},
    update::keys,
};
use crossterm::event::{KeyCode, KeyEvent};

pub(super) fn open(model: &mut Model, at_start: bool) -> Vec<Cmd> {
    model.overlay = Some(Overlay::WorkspacePicker {
        selected: None,
        items: Vec::new(),
        at_start,
    });
    let request = model.allocate(|id| Request::Workspaces { id });
    model.dirty = true;
    vec![Cmd::Get(request)]
}

pub(super) fn apply(model: &mut Model, values: Vec<compozy_client::types::Workspace>) {
    if let Some(Overlay::WorkspacePicker {
        selected, items, ..
    }) = &mut model.overlay
    {
        *items = values
            .into_iter()
            .map(|item| WorkspaceRef {
                id: item.id,
                name: item.name,
                root_dir: item.root_dir,
            })
            .collect();
        *selected = (!items.is_empty()).then_some(0);
        model.dirty = true;
    }
}

pub(super) fn key(model: &mut Model, key: KeyEvent) -> Vec<Cmd> {
    if key.code == KeyCode::Esc
        && let Some(candidate) = model.startup_candidate.clone()
    {
        model.start_workspace_onboarding(candidate);
        return Vec::new();
    }
    let Some(Overlay::WorkspacePicker {
        selected,
        items,
        at_start,
    }) = &mut model.overlay
    else {
        return Vec::new();
    };
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if !items.is_empty() {
                *selected = Some((selected.unwrap_or_default() + 1).min(items.len() - 1));
                model.dirty = true;
            }
            Vec::new()
        }
        KeyCode::Char('k') | KeyCode::Up => {
            *selected = Some(selected.unwrap_or_default().saturating_sub(1));
            model.dirty = true;
            Vec::new()
        }
        KeyCode::PageDown => {
            if !items.is_empty() {
                *selected = Some((selected.unwrap_or_default() + 10).min(items.len() - 1));
                model.dirty = true;
            }
            Vec::new()
        }
        KeyCode::PageUp => {
            *selected = Some(selected.unwrap_or_default().saturating_sub(10));
            model.dirty = true;
            Vec::new()
        }
        KeyCode::Enter => selected
            .and_then(|index| items.get(index))
            .cloned()
            .map_or_else(Vec::new, |workspace| switch(model, workspace)),
        KeyCode::Esc => {
            if *at_start && items.is_empty() {
                keys::quit(model)
            } else {
                model.overlay = None;
                model.dirty = true;
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

pub(super) fn switch(model: &mut Model, workspace: WorkspaceRef) -> Vec<Cmd> {
    let mut commands = model.all_stop_cmds();
    let onboarding_candidate = match &model.overlay {
        Some(Overlay::WorkspaceOnboarding { candidate, .. }) => Some(candidate.clone()),
        _ => None,
    };
    let late_writes = model
        .pending
        .iter()
        .filter_map(|(id, pending)| match pending {
            crate::app::PendingKind::Request(request) if request.is_write() => {
                Some((*id, request.clone()))
            }
            _ => None,
        })
        .collect();
    let mut settings = model.settings.clone();
    settings.workspace = Some(workspace);
    let mode = model.mode;
    let mut next = Model::new(settings, mode);
    next.late_writes = late_writes;
    let boot_commands = next.initial_cmds();
    if let Some(candidate) = onboarding_candidate {
        next.start_workspace_onboarding(candidate);
        if let Some(Overlay::WorkspaceOnboarding { booting, .. }) = &mut next.overlay {
            *booting = true;
        }
        next.startup_boot_pending = next
            .pending
            .iter()
            .filter_map(|(id, pending)| match pending {
                crate::app::PendingKind::Request(
                    Request::Sessions { .. } | Request::Runs { .. } | Request::Overview { .. },
                ) => Some(*id),
                _ => None,
            })
            .collect();
    }
    commands.extend(boot_commands);
    *model = next;
    commands
}
