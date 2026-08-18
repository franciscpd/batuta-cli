use compozy_client::StreamCursor;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cmd {
    FetchPage { before_sequence: Option<i64> },
    StartStream(StreamCursor),
    StopStream,
    ClearFooterAfter(Duration),
    Render,
    Quit,
}
