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
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(if mode == LayoutMode::Compact {
            [Constraint::Length(30), Constraint::Min(50)]
        } else {
            [Constraint::Percentage(40), Constraint::Percentage(60)]
        })
        .split(rows[1]);
    if mode == LayoutMode::Compact {
        let shown = if model.focus == Panel::Detail {
            model.last_list_focus
        } else {
            model.focus
        };
        let (sessions, runs, attention) = match shown {
            Panel::Sessions => (columns[0], zero, zero),
            Panel::Runs => (zero, columns[0], zero),
            Panel::Attention => (zero, zero, columns[0]),
            Panel::Detail => (columns[0], zero, zero),
        };
        Areas {
            mode,
            header: rows[0],
            sessions,
            runs,
            attention,
            detail: columns[1],
            footer: rows[2],
        }
    } else {
        let left = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(40),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
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
