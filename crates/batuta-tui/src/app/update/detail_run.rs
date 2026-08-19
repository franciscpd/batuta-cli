use crate::{
    app::{Confirm, Detail, Model},
    cmd::{Cmd, Request},
};
use compozy_client::{GateDecision, RunControl};
use crossterm::event::{KeyCode, KeyEvent};

use crate::app::panels::detail_run::{self, Target};

pub(super) fn key(model: &mut Model, key: KeyEvent) -> Vec<Cmd> {
    let terminal_status = match &model.detail {
        Detail::Run(detail) => detail.run.as_ref().map(|value| value.run.status.clone()),
        _ => return Vec::new(),
    };
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => move_selection(model, 1, false),
        KeyCode::Up => move_selection(model, 1, true),
        KeyCode::PageDown => move_selection(model, 10, false),
        KeyCode::PageUp => move_selection(model, 10, true),
        KeyCode::Char('g') => set_selection(model, 0),
        KeyCode::Char('G') => {
            let last = run_detail(model).map_or(0, |detail| {
                detail_run::targets(detail).len().saturating_sub(1)
            });
            set_selection(model, last)
        }
        KeyCode::Enter => open_selected(model),
        KeyCode::Char('p') => control(model, terminal_status.as_deref(), RunControl::Pause),
        KeyCode::Char('u') => control(model, terminal_status.as_deref(), RunControl::Resume),
        KeyCode::Char('k') => control(model, terminal_status.as_deref(), RunControl::Kill),
        KeyCode::Char('a') => approve(model, GateDecision::Approve),
        KeyCode::Char('x') => approve(model, GateDecision::Reject),
        _ => Vec::new(),
    }
}

fn run_detail(model: &Model) -> Option<&crate::app::RunDetail> {
    match &model.detail {
        Detail::Run(detail) => Some(detail),
        _ => None,
    }
}

fn run_detail_mut(model: &mut Model) -> Option<&mut crate::app::RunDetail> {
    match &mut model.detail {
        Detail::Run(detail) => Some(detail),
        _ => None,
    }
}

fn set_selection(model: &mut Model, selection: usize) -> Vec<Cmd> {
    if let Some(detail) = run_detail_mut(model) {
        detail.selection = selection;
        model.dirty = true;
    }
    Vec::new()
}

fn move_selection(model: &mut Model, amount: usize, up: bool) -> Vec<Cmd> {
    let Some(detail) = run_detail_mut(model) else {
        return Vec::new();
    };
    let len = detail_run::targets(detail).len();
    if len > 0 {
        detail.selection = if up {
            detail.selection.saturating_sub(amount)
        } else {
            (detail.selection + amount).min(len - 1)
        };
        model.dirty = true;
    }
    Vec::new()
}

fn open_selected(model: &mut Model) -> Vec<Cmd> {
    let target = run_detail(model).and_then(detail_run::selected_target);
    match target {
        Some(Target::Child(id)) => super::keys::open_run_id(model, id.to_owned()),
        Some(Target::Session(id)) => super::keys::open_session_id(model, id.to_owned()),
        _ => Vec::new(),
    }
}

fn control(model: &mut Model, status: Option<&str>, control: RunControl) -> Vec<Cmd> {
    let Some(run_id) = run_detail(model).map(|detail| detail.run_id.clone()) else {
        return Vec::new();
    };
    if status.is_some_and(detail_run::terminal) {
        model.set_sticky_toast(format!("run is already {}", status.unwrap_or_default()));
        return Vec::new();
    }
    let verb = match control {
        RunControl::Pause => "pause run",
        RunControl::Resume => "resume run",
        RunControl::Cancel => "cancel run",
        RunControl::Kill => "kill run",
    };
    if model.refuse_write(verb) {
        return Vec::new();
    }
    if control == RunControl::Kill {
        let prompt = format!("kill run {run_id}? Enter/Esc");
        run_detail_mut(model).unwrap().confirm = Some(Confirm { prompt });
        model.dirty = true;
        return Vec::new();
    }
    post_control(model, control)
}

fn post_control(model: &mut Model, control: RunControl) -> Vec<Cmd> {
    let Some(workspace) = model.workspace.as_ref().map(|value| value.id.clone()) else {
        return Vec::new();
    };
    let Some(run) = run_detail(model).map(|detail| detail.run_id.clone()) else {
        return Vec::new();
    };
    vec![Cmd::Post(model.allocate(|id| Request::RunControl {
        id,
        workspace,
        run,
        control,
    }))]
}

pub(super) fn confirm(model: &mut Model, key: KeyCode) -> Vec<Cmd> {
    match key {
        KeyCode::Enter => {
            if let Some(detail) = run_detail_mut(model) {
                detail.confirm = None;
            }
            post_control(model, RunControl::Kill)
        }
        KeyCode::Esc => {
            if let Some(detail) = run_detail_mut(model) {
                detail.confirm = None;
                model.dirty = true;
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

fn approve(model: &mut Model, decision: GateDecision) -> Vec<Cmd> {
    let gate = run_detail(model).and_then(detail_run::selected_target);
    let Some(Target::Gate { id: gate_id, .. }) = gate else {
        model.set_sticky_toast("select a gate first");
        return Vec::new();
    };
    let gate_id = gate_id.to_owned();
    let Some(workspace) = model.workspace.as_ref().map(|value| value.id.clone()) else {
        return Vec::new();
    };
    let run = run_detail(model).unwrap().run_id.clone();
    vec![Cmd::Post(model.allocate(|id| Request::RunApprove {
        id,
        workspace,
        run,
        gate_id,
        decision,
    }))]
}
