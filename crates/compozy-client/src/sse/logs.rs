use super::{EventStream, Frame, LogCursor, ReconnectPolicy, StreamEvent, StreamRequest};
use crate::{Client, LogQuery, types::LogEvent};
use futures_util::Stream;
use tokio::sync::watch;

fn map(frame: &Frame) -> Option<LogEvent> {
    serde_json::from_str(&frame.data).ok()
}

impl Client {
    pub fn logs_stream(
        &self,
        query: &LogQuery<'_>,
        cursor: watch::Receiver<LogCursor>,
        policy: ReconnectPolicy,
    ) -> impl Stream<Item = StreamEvent<LogEvent>> + Send + 'static {
        let mut request = StreamRequest::new("/api/logs/stream", "logs stream");
        request.query = form_urlencoded::parse(query.encode().as_bytes())
            .filter_map(|(name, value)| {
                let name = match name.as_ref() {
                    "workspace_id" => "workspace_id",
                    "session_id" => "session_id",
                    "run" => "run",
                    "error_only" => "error_only",
                    "after_seq" => return None,
                    "limit" => "limit",
                    _ => unreachable!("known log query field"),
                };
                Some((name, value.into_owned()))
            })
            .collect();
        EventStream::open(self.clone(), request, cursor, policy, map)
    }
}
