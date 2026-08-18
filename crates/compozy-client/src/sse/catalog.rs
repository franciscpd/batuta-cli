use super::{EventStream, Frame, NoCursor, ReconnectPolicy, StreamEvent, StreamRequest};
use crate::{Client, types::CatalogEvent};
use futures_util::Stream;
use tokio::sync::watch;

fn map(frame: &Frame) -> Option<CatalogEvent> {
    (frame.event == "session_catalog_changed")
        .then(|| serde_json::from_str(&frame.data).ok())
        .flatten()
}

impl Client {
    pub fn catalog_stream(
        &self,
        cursor: watch::Receiver<NoCursor>,
        policy: ReconnectPolicy,
    ) -> impl Stream<Item = StreamEvent<CatalogEvent>> + Send + 'static {
        let mut request = StreamRequest::new("/api/sessions/catalog-stream", "catalog stream");
        request.retry_server_errors = false;
        EventStream::open(self.clone(), request, cursor, policy, map)
    }
}
