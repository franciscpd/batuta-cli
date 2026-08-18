use compozy_client::{TranscriptEvent, types::TranscriptPage};
use crossterm::event::{KeyEvent, MouseEvent};

#[derive(Debug)]
pub enum Msg {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Tick,
    Page(Result<TranscriptPage, String>),
    Stream(TranscriptEvent),
    ClearFooter,
    Quit,
}
