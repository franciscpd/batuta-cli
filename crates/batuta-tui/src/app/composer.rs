use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};
use tui_textarea::{TextArea, WrapMode};

pub trait Composer {
    fn insert(&mut self, key: KeyEvent) -> bool;
    fn text(&self) -> String;
    fn clear(&mut self);
    fn is_empty(&self) -> bool;
    fn render(&self, area: Rect, frame: &mut Frame<'_>);
}

#[derive(Clone, Debug)]
pub struct TextareaComposer {
    textarea: TextArea<'static>,
}

impl Default for TextareaComposer {
    fn default() -> Self {
        let mut textarea = TextArea::default();
        textarea.set_wrap_mode(WrapMode::WordOrGlyph);
        textarea.set_min_rows(3);
        textarea.set_max_rows(3);
        textarea.set_cursor_line_style(ratatui::style::Style::default());
        Self { textarea }
    }
}

impl TextareaComposer {
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        let lines = text.split('\n').map(str::to_owned).collect::<Vec<_>>();
        let row = lines.len().saturating_sub(1);
        let col = lines.last().map_or(0, |line| line.chars().count());
        self.textarea.set_lines(lines, (row, col));
    }

    pub fn cursor(&self) -> (usize, usize) {
        self.textarea.cursor()
    }
}

impl Composer for TextareaComposer {
    fn insert(&mut self, key: KeyEvent) -> bool {
        self.textarea.input(key)
    }

    fn text(&self) -> String {
        self.textarea.lines().join("\n")
    }

    fn clear(&mut self) {
        self.textarea.clear();
    }

    fn is_empty(&self) -> bool {
        self.textarea.is_empty()
    }

    fn render(&self, area: Rect, frame: &mut Frame<'_>) {
        frame.render_widget(&self.textarea, area);
    }
}

#[derive(Clone, Debug, Default)]
pub struct ComposerState {
    editor: TextareaComposer,
    pub focused: bool,
}

impl ComposerState {
    pub fn insert(&mut self, key: KeyEvent) -> bool {
        self.editor.insert(key)
    }

    pub fn text(&self) -> String {
        self.editor.text()
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.editor.set_text(text);
    }

    pub fn cursor(&self) -> (usize, usize) {
        self.editor.cursor()
    }

    pub fn clear(&mut self) {
        self.editor.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.editor.is_empty()
    }

    pub fn render(&self, area: Rect, frame: &mut Frame<'_>) {
        self.editor.render(area, frame);
    }
}
