use super::engine::handshake;
use super::{EventStream, Frame, NoCursor, ReconnectPolicy, StreamEvent, StreamRequest};
use crate::{Client, Error, types::CatalogEvent};
use futures_util::Stream;
use tokio::sync::watch;

fn map(frame: &Frame) -> Option<CatalogEvent> {
    (frame.event == "session_catalog_changed")
        .then(|| serde_json::from_str(&frame.data).ok())
        .flatten()
}

impl Client {
    pub async fn catalog_stream_handshake(&self) -> Result<(), Error> {
        let mut request = StreamRequest::new("/api/sessions/catalog-stream", "catalog stream");
        // Doctor is a one-shot diagnostic: a failed handshake is terminal for
        // this invocation, even though the long-lived catalog stream retries it.
        request.retry_server_errors = false;
        handshake(self, &request).await
    }

    pub fn catalog_stream(
        &self,
        cursor: watch::Receiver<NoCursor>,
        policy: ReconnectPolicy,
    ) -> impl Stream<Item = StreamEvent<CatalogEvent>> + Send + 'static {
        EventStream::open(self.clone(), catalog_stream_request(), cursor, policy, map)
    }
}

fn catalog_stream_request() -> StreamRequest {
    let mut request = StreamRequest::new("/api/sessions/catalog-stream", "catalog stream");
    request.retry_server_errors = true;
    request
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ut_009_catalog_stream_request_retries_server_errors() {
        assert!(catalog_stream_request().retry_server_errors);
    }
}
