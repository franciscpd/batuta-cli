use super::{EventStream, Frame, ReconnectPolicy, StreamCursor, StreamEvent, StreamRequest};
use crate::{
    Client, Error,
    sessions::encode_segment,
    types::{SessionStopped, TranscriptDelta, TranscriptSnapshot},
};
use futures_util::{Stream, StreamExt};
use tokio::sync::watch;

pub struct ResetReason;

impl ResetReason {
    pub const FENCE_MISSING: &'static str = "fence_missing";
    pub const EPOCH_MISMATCH: &'static str = "epoch_mismatch";
    pub const GENERATION_MISMATCH: &'static str = "generation_mismatch";
    pub const SEQUENCE_RESET: &'static str = "sequence_reset";
}

#[derive(Debug)]
pub enum TranscriptEvent {
    Snapshot(TranscriptSnapshot),
    Delta(TranscriptDelta),
    Stopped(SessionStopped),
    ServerError(crate::types::ErrorPayload),
    Lost {
        attempt: u32,
        next_in: std::time::Duration,
        error: String,
        offline_after: u32,
    },
    Reconnected,
    Fatal(Error),
}

enum Data {
    Snapshot(TranscriptSnapshot),
    Delta(TranscriptDelta),
    Stopped(SessionStopped),
}

fn map(frame: &Frame) -> Option<Data> {
    match frame.event.as_str() {
        "transcript_snapshot" => serde_json::from_str(&frame.data).ok().map(Data::Snapshot),
        "transcript_delta" => serde_json::from_str(&frame.data).ok().map(Data::Delta),
        "session_stopped" => serde_json::from_str(&frame.data).ok().map(Data::Stopped),
        _ => None,
    }
}

impl Client {
    pub fn transcript_stream(
        &self,
        workspace: &str,
        session_id: &str,
        cursor: watch::Receiver<StreamCursor>,
        policy: ReconnectPolicy,
    ) -> impl Stream<Item = TranscriptEvent> + Send + 'static {
        let path = format!(
            "/api/workspaces/{}/sessions/{}/stream",
            encode_segment(workspace),
            encode_segment(session_id)
        );
        let mut request = StreamRequest::new(path, "transcript stream");
        request.stop_event = Some("session_stopped");
        EventStream::open(self.clone(), request, cursor, policy, map).map(|event| match event {
            StreamEvent::Event(Data::Snapshot(value)) => TranscriptEvent::Snapshot(value),
            StreamEvent::Event(Data::Delta(value)) => TranscriptEvent::Delta(value),
            StreamEvent::Event(Data::Stopped(value)) => TranscriptEvent::Stopped(value),
            StreamEvent::ServerError(value) => TranscriptEvent::ServerError(value),
            StreamEvent::Lost {
                attempt,
                next_in,
                error,
                offline_after,
            } => TranscriptEvent::Lost {
                attempt,
                next_in,
                error,
                offline_after,
            },
            StreamEvent::Reconnected => TranscriptEvent::Reconnected,
            StreamEvent::Fatal(value) => TranscriptEvent::Fatal(value),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cursor;

    #[test]
    fn policy_and_reset_contract() {
        let policy = ReconnectPolicy::default();
        assert_eq!(policy.min, std::time::Duration::from_millis(500));
        assert_eq!(policy.max, std::time::Duration::from_secs(10));
        assert_eq!(policy.offline_after, 5);
        assert_eq!(ResetReason::FENCE_MISSING, "fence_missing");
        assert_eq!(
            StreamCursor::default().query(),
            vec![("frames", "transcript".into())]
        );
    }
}
