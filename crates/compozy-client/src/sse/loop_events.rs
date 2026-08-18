use super::{EventStream, Frame, ReconnectPolicy, SeqCursor, StreamEvent, StreamRequest};
use crate::{Client, loops::loop_run_path, types::LoopEvent};
use async_stream::stream;
use futures_util::{Stream, StreamExt};
use tokio::sync::watch;

fn map(frame: &Frame) -> Option<LoopEvent> {
    let mut event: LoopEvent = serde_json::from_str(&frame.data).ok()?;
    if event.kind.is_empty() {
        event.kind = frame.event.clone();
    }
    Some(event)
}

impl Client {
    pub fn loop_run_events(
        &self,
        workspace: &str,
        id: &str,
        cursor: watch::Receiver<SeqCursor>,
        policy: ReconnectPolicy,
    ) -> impl Stream<Item = StreamEvent<LoopEvent>> + Send + 'static {
        let client = self.clone();
        let workspace = workspace.to_owned();
        let id = id.to_owned();
        stream! {
            if let Err(error) = client.loop_run(&workspace, &id).await {
                yield StreamEvent::Fatal(error);
                return;
            }
            let events = EventStream::open(
                client.clone(),
                StreamRequest::new(
                    format!("{}/events", loop_run_path(&workspace, &id)),
                    "loop run events stream",
                ),
                cursor,
                policy,
                map,
            );
            futures_util::pin_mut!(events);
            while let Some(event) = events.next().await {
                yield event;
            }
        }
    }
}
