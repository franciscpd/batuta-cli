use crate::{
    app::{Detail, Model, Panel},
    keymap::{self, Context},
};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};
pub fn contexts(model: &Model) -> Vec<Context> {
    if model.overlay.is_some() {
        return vec![Context::Overlays];
    }
    if model.composer_focused() {
        return vec![Context::Composer];
    }
    match model.focus {
        Panel::Sessions => vec![Context::Global, Context::Lists, Context::Sessions],
        Panel::Runs => vec![Context::Global, Context::Lists],
        Panel::Attention => vec![Context::Global, Context::Lists, Context::Attention],
        Panel::Detail => match model.detail {
            Detail::Session(_) => vec![Context::Global, Context::SessionDetail],
            Detail::Run(_) => vec![Context::Global, Context::RunDetail],
            Detail::Empty => vec![Context::Global],
        },
    }
}
pub fn render(model: &Model, frame: &mut Frame<'_>, area: Rect) {
    let text = if model.focus == Panel::Attention {
        crate::app::panels::attention::allowed_verbs(model)
            .unwrap_or_else(|| keymap::footer(&contexts(model), &model.settings.keymap))
    } else {
        keymap::footer(&contexts(model), &model.settings.keymap)
    };
    frame.render_widget(Paragraph::new(text).style(model.theme.muted), area);
}
