use crate::app::{AppMode, Model, Panel};
use ratatui::layout::{Constraint, Direction, Layout, Rect};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutMode {
    TooSmall,
    Compact,
    Full,
}
#[derive(Clone, Copy, Debug)]
pub struct Areas {
    pub mode: LayoutMode,
    pub header: Rect,
    pub sessions: Rect,
    pub runs: Rect,
    pub attention: Rect,
    pub detail: Rect,
    pub footer: Rect,
}

pub fn mode(width: u16, height: u16) -> LayoutMode {
    if width < 80 || height < 24 {
        LayoutMode::TooSmall
    } else if width < 100 {
        LayoutMode::Compact
    } else {
        LayoutMode::Full
    }
}
pub fn areas(area: Rect, model: &Model) -> Areas {
    let mode = mode(area.width, area.height);
    let zero = Rect::new(0, 0, 0, 0);
    if mode == LayoutMode::TooSmall {
        return Areas {
            mode,
            header: zero,
            sessions: zero,
            runs: zero,
            attention: zero,
            detail: area,
            footer: zero,
        };
    }
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if super::header::has_banner(model) {
                2
            } else {
                1
            }),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);
    if model.mode == AppMode::TailOnly {
        return Areas {
            mode,
            header: rows[0],
            sessions: zero,
            runs: zero,
            attention: zero,
            detail: rows[1],
            footer: rows[2],
        };
    }
    if mode == LayoutMode::Compact {
        let (sessions, runs, attention, detail) = match model.focus {
            Panel::Sessions => (rows[1], zero, zero, zero),
            Panel::Runs => (zero, rows[1], zero, zero),
            Panel::Attention => (zero, zero, rows[1], zero),
            Panel::Detail => (zero, zero, zero, rows[1]),
        };
        Areas {
            mode,
            header: rows[0],
            sessions,
            runs,
            attention,
            detail,
            footer: rows[2],
        }
    } else {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(rows[1]);
        let grown = contextual_panel_to_grow(model);
        let left = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                contextual_constraint(grown, Panel::Sessions),
                contextual_constraint(grown, Panel::Runs),
                contextual_constraint(grown, Panel::Attention),
            ])
            .split(columns[0]);
        Areas {
            mode,
            header: rows[0],
            sessions: left[0],
            runs: left[1],
            attention: left[2],
            detail: columns[1],
            footer: rows[2],
        }
    }
}

const COMPACT_PANEL_HEIGHT: u16 = 3;

fn contextual_constraint(grown: Option<Panel>, panel: Panel) -> Constraint {
    if grown == Some(panel) {
        Constraint::Fill(1)
    } else {
        Constraint::Length(COMPACT_PANEL_HEIGHT)
    }
}

fn contextual_panel_to_grow(model: &Model) -> Option<Panel> {
    let mut best = None;
    let mut best_relevance = 0;
    for panel in [Panel::Sessions, Panel::Runs, Panel::Attention] {
        let relevance = panel_relevance(model, panel);
        if relevance > best_relevance {
            best = Some(panel);
            best_relevance = relevance;
        }
    }
    best
}

fn panel_relevance(model: &Model, panel: Panel) -> u8 {
    if panel == Panel::Attention && !model.attention.is_empty() {
        4
    } else if model.focus == panel {
        3
    } else if panel == Panel::Runs
        && model
            .runs
            .items
            .iter()
            .any(|run| !is_terminal_run(&run.status))
    {
        2
    } else if panel_is_populated(model, panel) {
        1
    } else {
        0
    }
}

fn panel_is_populated(model: &Model, panel: Panel) -> bool {
    match panel {
        Panel::Sessions => !model.sessions.items.is_empty(),
        Panel::Runs => !model.runs.items.is_empty(),
        Panel::Attention => !model.attention.is_empty(),
        Panel::Detail => false,
    }
}

fn is_terminal_run(status: &str) -> bool {
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
